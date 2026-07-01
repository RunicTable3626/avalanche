# R8/ProGuard keep rules for the release (minified) build.
#
# Most libraries (Compose, AndroidX, Firebase, CameraX, kotlinx.serialization)
# ship their own consumer rules, so this file only needs the reflective/native
# surfaces R8 can't see through.

# --- JNA -------------------------------------------------------------------
# The UniFFI-generated bindings load libapp_core.so through JNA, which maps
# Kotlin interface/method names to native symbols at runtime via reflection and
# materializes Structure subclasses by field. R8 renaming/removing any of that
# breaks the native boundary at runtime (not at build time). Keep JNA whole.
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.* { public *; }
-dontwarn java.awt.**

# --- UniFFI-generated bindings (uniffi.app_core) ---------------------------
# JNA looks these up by name; the RustBuffer/ForeignBytes Structures, the
# library interface, and the callback interfaces must keep their names + members.
-keep class uniffi.** { *; }

# --- kotlinx.serialization -------------------------------------------------
# Belt-and-suspenders for the SharedPreferences JSON models; the library ships
# consumer rules but we keep the generated serializers explicitly.
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.**
-keepclassmembers class **$$serializer { *; }
-keepclasseswithmembers class * {
    kotlinx.serialization.KSerializer serializer(...);
}
