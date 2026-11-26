#include <security/pam_modules.h>
#include <security/pam_ext.h>
#include <string.h>

/* Supplies simulated password outcomes for the isolated PAM stack. */
PAM_EXTERN int pam_sm_authenticate(pam_handle_t *pamh, int flags, int argc,
                                   const char **argv)
{
    (void)flags;
    if (argc != 1)
        return PAM_SERVICE_ERR;
    if (strcmp(argv[0], "seed") == 0)
        return pam_set_item(pamh, PAM_AUTHTOK, "fixture-token") == PAM_SUCCESS
                   ? PAM_IGNORE : PAM_SYSTEM_ERR;
    if (strcmp(argv[0], "check") == 0) {
        const void *item = NULL;
        if (pam_get_item(pamh, PAM_AUTHTOK, &item) != PAM_SUCCESS || !item)
            return PAM_AUTH_ERR;
        return strcmp(item, "fixture-token") == 0 ? PAM_SUCCESS : PAM_AUTH_ERR;
    }
    if (strcmp(argv[0], "password") == 0) {
        const char *password = NULL;
        if (pam_get_authtok(pamh, PAM_AUTHTOK, &password, NULL) != PAM_SUCCESS || !password)
            return PAM_AUTH_ERR;
        return strcmp(password, "fixture-password") == 0 ? PAM_SUCCESS : PAM_AUTH_ERR;
    }
    return PAM_SERVICE_ERR;
}
