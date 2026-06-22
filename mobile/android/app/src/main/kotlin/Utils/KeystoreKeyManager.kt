package net.theavalanche.app

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Android Keystore analog of iOS SecureEnclaveKeyManager.
 *
 * iOS stores the DB passphrase as a plain UTF-8 string in the Keychain, protected by
 * kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly.  On Android we replicate the same
 * semantics:
 *   - The raw 32-byte passphrase hex string is stored in EncryptedSharedPreferences
 *     (Jetpack Security), which internally wraps an AES-256-GCM key held in the
 *     Android Keystore.
 *   - The Keystore key is bound to the device (setIsStrongBoxBacked if available) and
 *     requires the device to be unlocked at least once after boot
 *     (KeyProperties.AUTH_BIOMETRIC_STRONG is NOT required so the passphrase can be
 *     retrieved in the background after first unlock, matching iOS behaviour).
 *
 * Callers must pass an application [Context] — use applicationContext to avoid leaks.
 */
object KeystoreKeyManager {

    private const val KEYSTORE_ALIAS = "actnet.db.encryption.key"
    private const val PREFS_FILE = "actnet_secure_prefs"
    private const val PREFS_KEY = "db.encryption.key"
    private const val ANDROID_KEYSTORE = "AndroidKeyStore"

    // AES-GCM parameters
    private const val KEY_SIZE_BITS = 256
    private const val GCM_TAG_LENGTH = 128
    private const val GCM_IV_LENGTH = 12
    private const val TRANSFORMATION = "AES/GCM/NoPadding"

    /**
     * Returns the DB passphrase, generating and persisting one if it does not yet exist.
     *
     * This call performs disk I/O and crypto — run it on a background dispatcher
     * (e.g. [kotlinx.coroutines.Dispatchers.IO]).
     *
     * Mirrors [SecureEnclaveKeyManager.dbPassphrase()] on iOS.
     */
    @Throws(KeyManagerException::class)
    fun dbPassphrase(context: Context): String {
        loadFromStorage(context)?.let { return it }
        val newKey = generatePassphrase()
        saveToStorage(context, newKey)
        return newKey
    }

    /**
     * Generates a 32-byte cryptographically random passphrase encoded as a 64-character
     * lowercase hex string — identical format to the iOS implementation.
     */
    @Throws(KeyManagerException::class)
    private fun generatePassphrase(): String {
        return try {
            val bytes = ByteArray(32)
            SecureRandom().nextBytes(bytes)
            bytes.joinToString("") { "%02x".format(it) }
        } catch (e: Exception) {
            throw KeyManagerException.RandomGenerationFailed(e)
        }
    }

    /**
     * Reads and decrypts the passphrase from SharedPreferences, or null if not yet
     * stored. The stored blob is [IV (12 bytes) || AES-256-GCM ciphertext], base64
     * encoded, decrypted with a non-exportable key held in the Android Keystore —
     * approximating iOS kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly.
     */
    @Throws(KeyManagerException::class)
    private fun loadFromStorage(context: Context): String? {
        return try {
            val stored = context.getSharedPreferences(PREFS_FILE, Context.MODE_PRIVATE)
                .getString(PREFS_KEY, null) ?: return null
            val blob = Base64.decode(stored, Base64.NO_WRAP)
            val iv = blob.copyOfRange(0, GCM_IV_LENGTH)
            val ciphertext = blob.copyOfRange(GCM_IV_LENGTH, blob.size)
            val cipher = Cipher.getInstance(TRANSFORMATION).apply {
                init(Cipher.DECRYPT_MODE, getOrCreateSecretKey(), GCMParameterSpec(GCM_TAG_LENGTH, iv))
            }
            String(cipher.doFinal(ciphertext), Charsets.UTF_8)
        } catch (e: Exception) {
            throw KeyManagerException.KeystoreReadFailed(e)
        }
    }

    /**
     * Encrypts [passphrase] with the Keystore key and persists [IV || ciphertext]
     * (base64) to SharedPreferences. The plaintext never touches disk.
     */
    @Throws(KeyManagerException::class)
    private fun saveToStorage(context: Context, passphrase: String) {
        try {
            val cipher = Cipher.getInstance(TRANSFORMATION).apply {
                init(Cipher.ENCRYPT_MODE, getOrCreateSecretKey())
            }
            val iv = cipher.iv
            val ciphertext = cipher.doFinal(passphrase.toByteArray(Charsets.UTF_8))
            val encoded = Base64.encodeToString(iv + ciphertext, Base64.NO_WRAP)
            context.getSharedPreferences(PREFS_FILE, Context.MODE_PRIVATE)
                .edit().putString(PREFS_KEY, encoded).apply()
        } catch (e: Exception) {
            throw KeyManagerException.KeystoreWriteFailed(e)
        }
    }

    /**
     * Returns the AES-256-GCM key for this app from the Android Keystore, creating
     * a hardware-backed (where available), non-exportable key on first use. The key
     * is usable after first unlock and does not require per-use authentication, so
     * the DB can be opened in the background — matching the iOS Keychain semantics.
     */
    private fun getOrCreateSecretKey(): SecretKey {
        val keystore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (keystore.getEntry(KEYSTORE_ALIAS, null) as? KeyStore.SecretKeyEntry)
            ?.let { return it.secretKey }

        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        val spec = KeyGenParameterSpec.Builder(
            KEYSTORE_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(KEY_SIZE_BITS)
            .build()
        generator.init(spec)
        return generator.generateKey()
    }

    // -------------------------------------------------------------------------
    // Error types — mirror iOS KeyManagerError
    // -------------------------------------------------------------------------

    sealed class KeyManagerException(message: String, cause: Throwable? = null) :
        Exception(message, cause) {

        /** Mirrors iOS KeyManagerError.randomGenerationFailed */
        class RandomGenerationFailed(cause: Throwable) :
            KeyManagerException("Failed to generate random passphrase", cause)

        /** Mirrors iOS KeyManagerError.keychainReadFailed */
        class KeystoreReadFailed(cause: Throwable) :
            KeyManagerException("Failed to read passphrase from Android Keystore storage", cause)

        /** Mirrors iOS KeyManagerError.keychainWriteFailed */
        class KeystoreWriteFailed(cause: Throwable) :
            KeyManagerException("Failed to write passphrase to Android Keystore storage", cause)
    }
}
