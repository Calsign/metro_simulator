
bootstrap:
	bazel run //:gen_rust_project

# NOTE: bazel mobile-install doesn't seem to update native libraries
android-install:
	bazel build //mobile/android:app --fat_apk_cpu=arm64-v8a
	adb install build/bazel-bin/mobile/android/app.apk
	adb shell am start -a android.intent.action.MAIN -n com.calsignlabs.metro_simulator/android.app.NativeActivity

.PHONY: update
