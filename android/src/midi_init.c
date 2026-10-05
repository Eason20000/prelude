/* JNI bridge: hand the Android application context to the Rust MIDI core.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Called once from PreludeActivity.onCreate (UI thread, before GTK starts).
 * midir's Android backend reaches MidiManager through ndk-context, which
 * must be initialized with a valid (JavaVM*, jobject); ndk-context only
 * stores the pointers, so the context is global-ref'd here and lives as
 * long as the process (application context: safe by construction).
 */

#include <jni.h>

void prelude_android_init_midi(void *vm, void *context);

JNIEXPORT void JNICALL
Java_top_vikasmi_prelude_PreludeActivity_nativeInitMidi(JNIEnv *env,
                                                        jclass clazz,
                                                        jobject context)
{
  (void) clazz;

  JavaVM *vm = NULL;
  if ((*env)->GetJavaVM (env, &vm) != JNI_OK || vm == NULL)
    return;

  jobject global = (*env)->NewGlobalRef (env, context);
  if (global == NULL)
    return;

  prelude_android_init_midi ((void *) vm, (void *) global);
}
