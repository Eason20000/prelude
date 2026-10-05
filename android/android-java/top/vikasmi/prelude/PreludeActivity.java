package top.vikasmi.prelude;

import android.content.Context;
import android.os.Bundle;
import androidx.annotation.Keep;
import org.gtk.android.ToplevelActivity;

// SPDX-License-Identifier: GPL-3.0-only
//
// Application activity: the stock GTK activity plus a one-time MIDI
// bootstrap. onCreate runs on the UI thread strictly before main() starts
// on the GTK thread, so the Rust MIDI context is ready before any midir
// call can happen.
public class PreludeActivity extends ToplevelActivity {
    static {
        // No-op if the runtime already loaded it; guarantees our JNI entry
        // below resolves no matter who wins the load race.
        System.loadLibrary("prelude");
    }

    private static boolean midiInitialized = false;

    @Keep
    private static native void nativeInitMidi(Context context);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        if (!midiInitialized) {
            nativeInitMidi(getApplicationContext());
            midiInitialized = true;
        }
        super.onCreate(savedInstanceState);
    }
}
