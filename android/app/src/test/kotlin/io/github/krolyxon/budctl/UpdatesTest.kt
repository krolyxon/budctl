package io.github.krolyxon.budctl

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The release's version, compared the way Gradle makes the installed one's code. */
class UpdatesTest {
    @Test
    fun versionCodesFollowGradle() {
        assertEquals(105, Updater.versionCode("0.1.5"))
        assertEquals(10203, Updater.versionCode("1.2.3"))
        assertTrue(Updater.versionCode("0.2.0")!! > Updater.versionCode("0.1.99")!!)
    }

    @Test
    fun otherTagsAreNotVersions() {
        assertNull(Updater.versionCode("0.1"))
        assertNull(Updater.versionCode("0.1.x"))
        assertNull(Updater.versionCode("1.2.3.4"))
    }

    @Test
    fun notesAreReflowed() {
        val notes = "Bass boost works now.\r\n\r\n- Bass boost has three bands. `budctl bass\r\n  low <level>` sets" +
            " one.\r\n- With `--debug`, more.\r\n"
        assertEquals(
            "Bass boost works now.\n• Bass boost has three bands. budctl bass low <level> sets one.\n• With --debug, more.",
            Updater.reflow(notes),
        )
    }
}
