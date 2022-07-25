
bootstrap:
	bazel run //:gen_rust_project

# NOTE: bazel mobile-install doesn't seem to update native libraries
android-install:
# the performance boost from -c opt is huge and it seems worthwhile when deploying to the phone
	bazel build //mobile/android:app --fat_apk_cpu=arm64-v8a -c opt
	adb install build/bazel-bin/mobile/android/app.apk
	adb shell am start -a android.intent.action.MAIN -n com.calsignlabs.metro_simulator/android.app.NativeActivity

bench:
	bazel query "attr(tags, bench, kind(rust_binary, //...))" | xargs -L1 bazel run -c opt

.PHONY: bootstrap android-install bench
