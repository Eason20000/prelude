/* Android toolchain smoke test: minimal GTK4/libadwaita app.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * Proves that the Pixiewood pipeline (prepare → generate → build) produces
 * an installable APK before Prelude itself is ported. Intentionally
 * independent from Prelude sources.
 */

#include <adwaita.h>

static void
on_activate (AdwApplication *app,
             gpointer        user_data)
{
  AdwStatusPage *status_page;
  AdwToolbarView *toolbar_view;
  AdwHeaderBar *header_bar;
  AdwWindowTitle *title;
  AdwApplicationWindow *window;

  (void) user_data;

  status_page = ADW_STATUS_PAGE (adw_status_page_new ());
  adw_status_page_set_icon_name (status_page, "emblem-ok-symbolic");
  adw_status_page_set_title (status_page, "GTK4 runs on Android");
  adw_status_page_set_description (
    status_page,
    "This window is drawn by the GDK Android backend with libadwaita.");

  title = ADW_WINDOW_TITLE (adw_window_title_new ("Prelude Android smoke test", NULL));
  header_bar = ADW_HEADER_BAR (adw_header_bar_new ());
  adw_header_bar_set_title_widget (header_bar, GTK_WIDGET (title));

  toolbar_view = ADW_TOOLBAR_VIEW (adw_toolbar_view_new ());
  adw_toolbar_view_add_top_bar (toolbar_view, GTK_WIDGET (header_bar));
  adw_toolbar_view_set_content (toolbar_view, GTK_WIDGET (status_page));

  window = ADW_APPLICATION_WINDOW (adw_application_window_new (GTK_APPLICATION (app)));
  adw_application_window_set_content (window, GTK_WIDGET (toolbar_view));
  gtk_window_set_default_size (GTK_WINDOW (window), 360, 640);
  gtk_window_present (GTK_WINDOW (window));
}

int
main (int   argc,
      char *argv[])
{
  g_autoptr (AdwApplication) app = NULL;

  app = adw_application_new ("top.vikasmi.PreludeSmoke",
                             G_APPLICATION_DEFAULT_FLAGS);
  g_signal_connect (app, "activate", G_CALLBACK (on_activate), NULL);

  return g_application_run (G_APPLICATION (app), argc, argv);
}
