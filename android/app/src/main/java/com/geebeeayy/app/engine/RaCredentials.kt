package com.geebeeayy.app.engine

import android.content.Context

/**
 * The signed-in player's name and login token.
 *
 * **The password is never here.** RetroAchievements answers a successful login
 * with a token, and that is what gets kept - so a stolen preferences file
 * cannot be used to sign in anywhere else, and the password never touches
 * disk.
 *
 * Plain `SharedPreferences`, matching every other small store in this app.
 * Not `EncryptedSharedPreferences`: that pulls in androidx.security for a
 * value that is already scoped to this app's private storage, and a device
 * where another app can read that storage is a device where the key is
 * readable too.
 */
class RaCredentials(context: Context) {
    private val prefs = context.getSharedPreferences("retroachievements", Context.MODE_PRIVATE)

    fun read(): Pair<String, String>? {
        val user = prefs.getString(KEY_USER, null) ?: return null
        val token = prefs.getString(KEY_TOKEN, null) ?: return null
        return user to token
    }

    fun write(user: String, token: String) {
        prefs.edit().putString(KEY_USER, user).putString(KEY_TOKEN, token).apply()
    }

    fun clear() {
        prefs.edit().remove(KEY_USER).remove(KEY_TOKEN).apply()
    }

    private companion object {
        const val KEY_USER = "user"
        const val KEY_TOKEN = "token"
    }
}
