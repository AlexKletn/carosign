#include <security/pam_modules.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Provides simulated outcomes and call ordering for the isolated test. */
PAM_EXTERN int pam_sm_authenticate(pam_handle_t *pamh, int flags, int argc,
                                   const char **argv)
{
    (void)pamh;
    (void)flags;
    if (argc != 1)
        return PAM_SERVICE_ERR;
    const char *trace = getenv("CAROSIGN_TEST_PAM_TRACE");
    FILE *output = trace ? fopen(trace, "a") : NULL;
    if (!output)
        return PAM_SYSTEM_ERR;
    if (fprintf(output, "%s\n", argv[0]) < 0 || fclose(output) != 0)
        return PAM_SYSTEM_ERR;
    if (strcmp(argv[0], "bio-unavailable") == 0)
        return PAM_AUTHINFO_UNAVAIL;
    if (strcmp(argv[0], "bio-no-match") == 0 || strcmp(argv[0], "locked") == 0)
        return PAM_AUTH_ERR;
    return PAM_SUCCESS;
}
