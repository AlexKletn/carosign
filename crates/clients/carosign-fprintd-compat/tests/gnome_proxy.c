#include <gio/gio.h>
#include <string.h>

/* Exercises the GDBusProxy used by GNOME Settings. */
int main(void) {
    GError *error = NULL;
    GDBusProxy *proxy = g_dbus_proxy_new_for_bus_sync(
        G_BUS_TYPE_SESSION, G_DBUS_PROXY_FLAGS_NONE, NULL,
        "net.reactivated.Fprint", "/net/reactivated/Fprint/Device/0",
        "net.reactivated.Fprint.Device", NULL, &error);
    if (!proxy) { g_printerr("%s\n", error->message); return 1; }
    GVariant *name = g_dbus_proxy_get_cached_property(proxy, "name");
    GVariant *stages = g_dbus_proxy_get_cached_property(proxy, "num-enroll-stages");
    int ok = name && stages && g_variant_is_of_type(name, G_VARIANT_TYPE_STRING)
        && strlen(g_variant_get_string(name, NULL)) > 0
        && g_variant_is_of_type(stages, G_VARIANT_TYPE_INT32)
        && g_variant_get_int32(stages) == 3;
    if (name) g_variant_unref(name);
    if (stages) g_variant_unref(stages);
    g_object_unref(proxy);
    return ok ? 0 : 1;
}
