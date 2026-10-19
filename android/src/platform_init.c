/* JNI bridge for the FluidSynth platform bootstrap.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * These are deliberately thin: string conversion and a call into Rust. All
 * engine work stays on the GTK main loop (see PreludeActivity).
 */

#include <jni.h>
#include <stddef.h>

void prelude_android_set_soundfont_path(const char *path);
void prelude_android_set_active(int active);
void prelude_android_shutdown(void);

JNIEXPORT void JNICALL
Java_top_vikasmi_prelude_PreludeActivity_nativeSetSoundfontPath(JNIEnv *env,
                                                                jclass clazz,
                                                                jstring path)
{
  (void) clazz;
  if (path == NULL) {
    prelude_android_set_soundfont_path(NULL);
    return;
  }

  const char *utf8 = (*env)->GetStringUTFChars (env, path, NULL);
  if (utf8 == NULL) {
    prelude_android_set_soundfont_path(NULL);
    return;
  }
  prelude_android_set_soundfont_path (utf8);
  (*env)->ReleaseStringUTFChars (env, path, utf8);
}

JNIEXPORT void JNICALL
Java_top_vikasmi_prelude_PreludeActivity_nativeSetActive(JNIEnv *env,
                                                         jclass clazz,
                                                         jboolean active)
{
  (void) env;
  (void) clazz;
  prelude_android_set_active (active == JNI_TRUE ? 1 : 0);
}

JNIEXPORT void JNICALL
Java_top_vikasmi_prelude_PreludeActivity_nativeShutdown(JNIEnv *env, jclass clazz)
{
  (void) env;
  (void) clazz;
  prelude_android_shutdown ();
}
