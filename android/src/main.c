/* Android entry shim for Prelude.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Pixiewood requires an exposed `main(int, char**, char**)` that ends in
 * `g_application_run` (Meson `android_exe_type: 'application'` links this
 * object as the `.so` the JVM launcher loads). All application logic,
 * including the `GApplication`, lives in the Rust staticlib; this shim only
 * forwards to it. rustc cannot emit this C-ABI entry itself, which is why
 * the shim exists instead of a Rust `main`.
 */

int prelude_android_main (int argc, char **argv);

int
main (int   argc,
      char *argv[],
      char *envp[])
{
  (void) envp;
  return prelude_android_main (argc, argv);
}
