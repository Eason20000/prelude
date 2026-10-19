package top.vikasmi.prelude;

import android.content.Context;
import android.content.res.AssetManager;
import android.os.Bundle;
import android.util.Log;
import androidx.annotation.Keep;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import org.gtk.android.ToplevelActivity;

// SPDX-License-Identifier: GPL-3.0-only
//
// Application activity: the stock GTK activity plus the MIDI and audio
// platform bootstrap. onCreate runs on the UI thread strictly before main()
// starts on the GTK thread, so the Rust MIDI context is ready before any
// midir call can happen.
public class PreludeActivity extends ToplevelActivity {
    private static final String TAG = "Prelude";
    private static final String SOUNDFONT_ASSET =
            "share/prelude/soundfonts/GeneralUser-GS.sf2";
    private static final String SOUNDFONT_NAME = "GeneralUser-GS-2.0.3.sf2";

    static {
        // No-op if the runtime already loaded it; guarantees our JNI entry
        // below resolves no matter who wins the load race.
        System.loadLibrary("prelude");
    }

    private static boolean midiInitialized = false;

    @Keep
    private static native void nativeInitMidi(Context context);

    @Keep
    private static native void nativeSetSoundfontPath(String path);

    @Keep
    private static native void nativeSetActive(boolean active);

    @Keep
    private static native void nativeShutdown();

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        if (!midiInitialized) {
            nativeInitMidi(getApplicationContext());
            midiInitialized = true;
        }
        prepareSoundfont();
        super.onCreate(savedInstanceState);
    }

    /**
     * Copy the bundled SoundFont to app-private storage once. Rust then uses
     * the same path-based fluid_synth_sfload() as desktop; no Android-only
     * SoundFont loader or second synth code path is involved.
     */
    private void prepareSoundfont() {
        final Context context = getApplicationContext();
        Thread worker = new Thread(() -> {
            File directory = new File(context.getFilesDir(), "soundfonts");
            File target = new File(directory, SOUNDFONT_NAME);
            if (target.isFile() && target.length() > 0) {
                nativeSetSoundfontPath(target.getAbsolutePath());
                return;
            }

            File temporary = new File(directory, SOUNDFONT_NAME + ".part");
            try {
                if (!directory.isDirectory() && !directory.mkdirs()) {
                    throw new IOException("cannot create " + directory);
                }
                AssetManager assets = context.getAssets();
                try (InputStream input = assets.open(SOUNDFONT_ASSET);
                     FileOutputStream output = new FileOutputStream(temporary)) {
                    byte[] buffer = new byte[1024 * 1024];
                    int read;
                    while ((read = input.read(buffer)) != -1) {
                        output.write(buffer, 0, read);
                    }
                }
                if (!temporary.renameTo(target)) {
                    throw new IOException("cannot rename " + temporary);
                }
            } catch (IOException error) {
                Log.e(TAG, "cannot prepare bundled SoundFont", error);
            } finally {
                if (temporary.isFile()) {
                    //noinspection ResultOfMethodCallIgnored
                    temporary.delete();
                }
            }
            // On failure this points at a missing file; FluidSynth reports a
            // normal load error and the GTK layer shows it as a toast.
            nativeSetSoundfontPath(target.getAbsolutePath());
        }, "prelude-soundfont");
        worker.setDaemon(true);
        worker.start();
    }

    @Override
    protected void onPause() {
        nativeSetActive(false);
        super.onPause();
    }

    @Override
    protected void onResume() {
        super.onResume();
        nativeSetActive(true);
    }

    @Override
    protected void onDestroy() {
        nativeShutdown();
        super.onDestroy();
    }
}
