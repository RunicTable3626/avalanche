package net.theavalanche.app

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.jsoup.Jsoup
import java.net.HttpURLConnection
import java.net.URL

/** Raw, Sendable preview data fetched on the sender's device (docs/35). The
 *  ViewModel uploads [imageData] as an encrypted attachment and builds the
 *  wire `LinkPreviewFfi` — this stays free of FFI types. */
data class FetchedPreview(
    val url: String,
    val title: String,
    val description: String,
    val imageData: ByteArray?,
)

private const val USER_AGENT = "Mozilla/5.0 (compatible; AvalancheBot/1.0)"
private const val TIMEOUT_MS = 8000

private val URL_IN_TEXT = Regex("""(https?://|www\.)\S+""", RegexOption.IGNORE_CASE)

/** First http(s) URL in [body], trailing punctuation trimmed; `www.` gets a scheme.
 *  (Plain-text URL detection — regex is the right tool here, unlike HTML parsing.) */
fun firstUrlIn(body: String): String? {
    val m = URL_IN_TEXT.find(body) ?: return null
    var raw = m.value
    raw = raw.dropLast(raw.takeLastWhile { it in ".,;:!?)]}\"'" }.length)
    if (raw.isEmpty()) return null
    return if (raw.startsWith("www", ignoreCase = true)) "http://$raw" else raw
}

/**
 * Fetch the page at the first URL in [body] and extract OpenGraph metadata
 * (docs/35 "Link previews") with Jsoup — the JVM-standard HTML parser, which
 * handles real-world markup, attribute-order/quoting variants, entity decoding,
 * and relative-URL resolution (the iOS analog is `LPMetadataProvider`). The
 * sender's device does this; the recipient never fetches the URL. Best-effort —
 * returns null on no URL / fetch failure.
 */
suspend fun fetchLinkPreview(body: String): FetchedPreview? = withContext(Dispatchers.IO) {
    val pageUrl = firstUrlIn(body) ?: return@withContext null
    // Jsoup.connect fetches (following redirects, honoring charset, capping body
    // size by default) and parses in one step.
    val doc = runCatching {
        Jsoup.connect(pageUrl)
            .userAgent(USER_AGENT)
            .timeout(TIMEOUT_MS)
            .followRedirects(true)
            .get()
    }.getOrNull() ?: return@withContext null

    val title = doc.selectFirst("""meta[property="og:title"]""")?.attr("content")?.ifBlank { null }
        ?: doc.title().ifBlank { null }
        ?: ""
    val description = doc.selectFirst("""meta[property="og:description"]""")?.attr("content")?.ifBlank { null }
        ?: doc.selectFirst("""meta[name="description"]""")?.attr("content")?.ifBlank { null }
        ?: ""
    // absUrl resolves a relative og:image against the (possibly redirected) page URL.
    val imageUrl = doc.selectFirst("""meta[property="og:image"]""")?.absUrl("content")?.ifBlank { null }
    val imageData = imageUrl?.let { runCatching { httpGetBytes(it, maxBytes = 4 * 1024 * 1024) }.getOrNull() }

    FetchedPreview(pageUrl, title, description, imageData)
}

/** Fetch the og:image bytes (Jsoup is for HTML; images are raw bytes). Refuses
 *  responses over [maxBytes]. */
private fun httpGetBytes(url: String, maxBytes: Int): ByteArray? {
    val c = URL(url).openConnection() as HttpURLConnection
    c.instanceFollowRedirects = true
    c.connectTimeout = TIMEOUT_MS
    c.readTimeout = TIMEOUT_MS
    c.setRequestProperty("User-Agent", USER_AGENT)
    return try {
        if (c.responseCode !in 200..299) return null
        val out = java.io.ByteArrayOutputStream()
        val buf = ByteArray(16 * 1024)
        var total = 0
        c.inputStream.use { input ->
            while (true) {
                val n = input.read(buf)
                if (n < 0) break
                total += n
                if (total > maxBytes) return null
                out.write(buf, 0, n)
            }
        }
        out.toByteArray()
    } finally {
        c.disconnect()
    }
}
