cmake_minimum_required(VERSION 3.22.1)
project(geebeeayy_ra C)

set(CMAKE_C_STANDARD 99)

# rcheevos is a git submodule, pinned. It is MIT-licensed C from
# RetroAchievements, and it lives here rather than in core/ on purpose: core/
# is Rust with two dependencies and no C, and deciding what an achievement
# means is not emulation. See docs/achievements.md.
set(RCHEEVOS ${CMAKE_CURRENT_SOURCE_DIR}/rcheevos)

# Only what a cartridge-based console needs. The disc, encryption and zip
# readers exist for CD systems and cost size and a dependency each; a GBA ROM
# is a flat file, so they are compiled out rather than carried.
add_compile_definitions(
    RC_HASH_NO_DISC
    RC_HASH_NO_ENCRYPTED
    RC_HASH_NO_ZIP
    RC_DISABLE_LUA
)

file(GLOB RCHEEVOS_SOURCES
    ${RCHEEVOS}/src/api/*.c
    ${RCHEEVOS}/src/client/*.c
    ${RCHEEVOS}/src/hash/*.c
    ${RCHEEVOS}/src/runtime/*.c
    ${RCHEEVOS}/src/util/*.c
)
# The RAIntegration path is Windows-only: it loads a DLL to talk to the
# achievement editor, and it does not compile for Android.
list(FILTER RCHEEVOS_SOURCES EXCLUDE REGEX "rc_client_raintegration\\.c$")
# rc_libretro is the glue for libretro cores and includes libretro.h, which is
# not here and never will be - this app links its own emulator directly.
list(FILTER RCHEEVOS_SOURCES EXCLUDE REGEX "rc_libretro\\.c$")
# Excluded by the RC_HASH_NO_* defines above, so their objects would be empty
# at best and pull in missing headers at worst.
list(FILTER RCHEEVOS_SOURCES EXCLUDE REGEX "(hash_disc|hash_encrypted|hash_zip|cdreader|aes)\\.c$")

add_library(geebeeayy_ra SHARED
    ra_bridge.c
    ${RCHEEVOS_SOURCES}
)

target_include_directories(geebeeayy_ra PRIVATE
    ${RCHEEVOS}/include
    ${RCHEEVOS}/src
    ${RCHEEVOS}/src/util
)

# 16 KB page alignment, which Play requires. NDK r27 and older link at 4 KB
# and ignore ANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES (r28+), so the flags go in
# explicitly; they are harmless on r28.
target_link_options(geebeeayy_ra PRIVATE
    -Wl,-z,max-page-size=16384
    -Wl,-z,common-page-size=16384
)

find_library(log-lib log)
target_link_libraries(geebeeayy_ra ${log-lib})
