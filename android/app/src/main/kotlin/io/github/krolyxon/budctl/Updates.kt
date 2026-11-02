package io.github.krolyxon.budctl

import android.app.Activity
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.PackageInstaller
import android.net.Uri
import android.provider.Settings
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import kotlin.concurrent.thread
import org.json.JSONException
import org.json.JSONObject

/** A budctl release on GitHub: its version, what Gradle makes its version code, its APK and notes. */
data class Release(val version: String, val code: Int, val apk: String, val sha256: String, val notes: String)

/** Where the update stands, for the APP page and the update dialog. */
sealed interface UpdateState {
    data object Idle : UpdateState

    data object Checking : UpdateState

    data object UpToDate : UpdateState

    data class Available(val release: Release) : UpdateState

    data class Downloading(val release: Release, val percent: Int) : UpdateState

    /** Handed to Android's installer, which asks before it installs. */
    data class Installing(val release: Release) : UpdateState

    /** [release]: the one to try again, when there is one. */
    data class Failed(val message: String, val release: Release?) : UpdateState
}

/** What the screen asks of the updater. */
interface UpdateActions {
    /** [automatic]: the daily check on opening the app, rather than Check now. */
    fun check(automatic: Boolean)

    fun download(release: Release)
}

/**
 * The Android app's update check: CLAUDE.md rule 1's one exception (docs/android.md#updates). It
 * goes online only when the owner has switched it on, or taps Check now: it asks GitHub for budctl's
 * latest release, and on Update downloads that release's APK, checks it against the release's
 * SHA-256, and hands it to Android's installer, which asks before installing and refuses an APK not
 * signed with budctl's key. Nothing about the owner or the buds is sent.
 *
 * [report] gets each new state on the main thread, with whether it comes from an automatic check.
 */
class Updater(
    private val activity: Activity,
    /** The installed app's version code, as Gradle makes it from the version. */
    private val installed: Int,
    private val report: (UpdateState, Boolean) -> Unit,
) : UpdateActions {
    private fun post(state: UpdateState, automatic: Boolean = false) = activity.runOnUiThread { report(state, automatic) }

    override fun check(automatic: Boolean) {
        if (!automatic) post(UpdateState.Checking)
        thread(name = "budctl-update", isDaemon = true) {
            val state = try {
                val release = latest()
                if (release.code > installed) UpdateState.Available(release) else UpdateState.UpToDate
            } catch (e: IOException) {
                UpdateState.Failed("Could not reach GitHub: ${e.message ?: e.javaClass.simpleName}", null)
            } catch (e: JSONException) {
                UpdateState.Failed("GitHub's answer made no sense: ${e.message}", null)
            }
            post(state, automatic)
        }
    }

    override fun download(release: Release) {
        post(UpdateState.Downloading(release, 0))
        thread(name = "budctl-update", isDaemon = true) {
            try {
                val apk = fetchApk(release)
                activity.runOnUiThread { install(release, apk) }
            } catch (e: IOException) {
                post(UpdateState.Failed("The download failed: ${e.message ?: e.javaClass.simpleName}", release))
            }
        }
    }

    /** An install the installer gave up on, as its result broadcast says. */
    fun failed(message: String?) = post(UpdateState.Failed("The install did not go through: ${message ?: "no reason given"}", null))

    /** The latest release: `releases/latest` skips drafts and pre-releases. */
    private fun latest(): Release {
        val json = JSONObject(read(LATEST))
        val tag = json.getString("tag_name")
        val assets = json.getJSONArray("assets").let { list -> List(list.length()) { list.getJSONObject(it) } }
        fun asset(name: String) =
            assets.firstOrNull { it.getString("name") == name }?.getString("browser_download_url")
                ?: throw IOException("release $tag has no $name")
        return Release(
            version = tag.removePrefix("v"),
            code = versionCode(tag.removePrefix("v")) ?: throw IOException("$tag is not a version budctl knows"),
            apk = asset("budctl.apk"),
            sha256 = asset("budctl.apk.sha256"),
            notes = reflow(json.optString("body")),
        )
    }

    /** The release's APK in the cache, checked against its SHA-256; a checked one from before is kept. */
    private fun fetchApk(release: Release): File {
        val expected = read(release.sha256).trim().substringBefore(' ').lowercase()
        if (expected.length != 64) throw IOException("the release's checksum is not a SHA-256")
        val dir = File(activity.cacheDir, "update").apply { mkdirs() }
        val apk = File(dir, "budctl-${release.version}.apk")
        if (apk.exists() && sha256(apk) == expected) return apk
        dir.listFiles()?.forEach { it.delete() }
        val connection = open(release.apk)
        try {
            val total = connection.contentLengthLong
            var done = 0L
            var shown = 0
            connection.inputStream.use { input ->
                apk.outputStream().use { output ->
                    val buffer = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buffer)
                        if (n < 0) break
                        output.write(buffer, 0, n)
                        done += n
                        val percent = if (total > 0) (done * 100 / total).toInt() else 0
                        if (percent != shown) {
                            shown = percent
                            post(UpdateState.Downloading(release, percent))
                        }
                    }
                }
            }
        } finally {
            connection.disconnect()
        }
        if (sha256(apk) != expected) {
            apk.delete()
            throw IOException("the APK did not match its SHA-256")
        }
        return apk
    }

    /** Hands the APK to Android's installer. Its answer comes to [INSTALL_RESULT]. */
    private fun install(release: Release, apk: File) {
        val packages = activity.packageManager
        if (!packages.canRequestPackageInstalls()) {
            // Android asks the owner to allow budctl to install apps, once.
            activity.startActivity(
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${activity.packageName}")),
            )
            post(UpdateState.Failed("Allow budctl to install apps, then try again", release))
            return
        }
        try {
            val installer = packages.packageInstaller
            val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
            params.setAppPackageName(activity.packageName)
            val id = installer.createSession(params)
            installer.openSession(id).use { session ->
                session.openWrite("budctl.apk", 0, apk.length()).use { output ->
                    apk.inputStream().use { it.copyTo(output) }
                    session.fsync(output)
                }
                val result = Intent(INSTALL_RESULT).setPackage(activity.packageName)
                // Mutable: the installer adds its status to the intent.
                val pending = PendingIntent.getBroadcast(activity, id, result, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE)
                session.commit(pending.intentSender)
            }
            post(UpdateState.Installing(release))
        } catch (e: IOException) {
            post(UpdateState.Failed("Android's installer refused the APK: ${e.message}", release))
        }
    }

    private fun open(url: String): HttpURLConnection {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.connectTimeout = 10_000
        connection.readTimeout = 30_000
        connection.setRequestProperty("Accept", "application/vnd.github+json")
        connection.setRequestProperty("User-Agent", "budctl")
        val status = connection.responseCode
        if (status != HttpURLConnection.HTTP_OK) {
            connection.disconnect()
            throw IOException("HTTP $status from ${URL(url).host}")
        }
        return connection
    }

    private fun read(url: String): String {
        val connection = open(url)
        try {
            return connection.inputStream.use { it.readBytes().decodeToString() }
        } finally {
            connection.disconnect()
        }
    }

    companion object {
        /** The only address the check asks; the APK comes from where GitHub points. */
        private const val LATEST = "https://api.github.com/repos/krolyxon/budctl/releases/latest"

        /** The installer's answer, sent to the app itself. */
        const val INSTALL_RESULT = "io.github.krolyxon.budctl.INSTALL_RESULT"

        /** `0.1.6` → 106, as app/build.gradle.kts makes the version code. */
        fun versionCode(version: String): Int? {
            val parts = version.split('.').map { it.toIntOrNull() ?: return null }
            return if (parts.size == 3) parts.fold(0) { code, part -> code * 100 + part } else null
        }

        /**
         * A release's notes (its CHANGELOG.md section, wrapped at 100 columns) as a phone shows
         * them: each paragraph and bullet on one line, bullets as dots, without code marks.
         */
        fun reflow(notes: String): String {
            val blocks = mutableListOf<String>()
            var joining = false
            for (raw in notes.replace("\r", "").lines()) {
                val line = raw.trim().replace("`", "")
                when {
                    line.isEmpty() -> joining = false
                    line.startsWith("- ") || line.startsWith("* ") -> {
                        blocks += "• " + line.drop(2)
                        joining = true
                    }
                    joining -> blocks[blocks.lastIndex] += " $line"
                    else -> {
                        blocks += line.trimStart('#', ' ')
                        joining = true
                    }
                }
            }
            return blocks.joinToString("\n")
        }

        private fun sha256(file: File): String {
            val digest = MessageDigest.getInstance("SHA-256")
            file.inputStream().use { input ->
                val buffer = ByteArray(64 * 1024)
                while (true) {
                    val n = input.read(buffer)
                    if (n < 0) break
                    digest.update(buffer, 0, n)
                }
            }
            return digest.digest().joinToString("") { "%02x".format(it) }
        }
    }
}
