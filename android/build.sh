#!/bin/sh
# Command-line Android build. Tools live in ~/android-cli (JDK 17, Android SDK, Gradle wrapper).
#   ./build.sh            build the debug APK
#   ./build.sh install    build and install on a phone connected over USB (USB debugging on)
export JAVA_HOME="$HOME/android-cli/jdk/Contents/Home"
export ANDROID_HOME="$HOME/android-cli/sdk"
cd "$(dirname "$0")" || exit 1
./gradlew assembleDebug || exit 1
APK=app/build/outputs/apk/debug/app-debug.apk
echo "built: $PWD/$APK"
[ "$1" = "install" ] && "$ANDROID_HOME/platform-tools/adb" install -r "$APK"
