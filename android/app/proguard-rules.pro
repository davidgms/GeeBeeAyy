# R8 rules for the release build.
#
# This file is referenced by build.gradle.kts but did not exist, while release
# builds have `isMinifyEnabled = true`. That is a silent break waiting to
# happen, because two native libraries find Kotlin by name:
#
#   - libgeebeeayy_core (Rust) exports `Java_com_geebeeayy_app_engine_GbaEngine_*`
#     and is bound to GbaEngine's `external fun`s by their names.
#   - libgeebeeayy_ra (C) calls *back* into RaEngine with GetStaticMethodID,
#     looking up `onServerRequest`, `onEvent`, `onLogin` and `onGameLoaded` by
#     exact name and signature. Nothing in Kotlin calls them, so R8 sees four
#     unused private methods and deletes them.
#
# A strip or a rename turns into an UnsatisfiedLinkError, or a null method id
# handed to CallStaticVoidMethod - a native crash - and it only ever happens in
# a release build. Debug never minifies, so nothing before release would show it.

# Natives: keep every class that declares one, and the names of those methods.
-keepclasseswithmembernames class * {
    native <methods>;
}

# The engine bindings, whole. Small classes, and every member is part of the
# contract with the native side.
-keep class com.geebeeayy.app.engine.GbaEngine { *; }
-keep class com.geebeeayy.app.engine.RaEngine { *; }

# The callbacks the C bridge looks up by name. Listed explicitly as well as
# covered above, so a future refactor that narrows the rule above cannot
# quietly drop them.
-keepclassmembers class com.geebeeayy.app.engine.RaEngine {
    static void onServerRequest(int, java.lang.String, java.lang.String);
    static void onEvent(int, int, java.lang.String, java.lang.String, int);
    static void onLogin(int, java.lang.String);
    static void onGameLoaded(int, java.lang.String, java.lang.String, int);
}
