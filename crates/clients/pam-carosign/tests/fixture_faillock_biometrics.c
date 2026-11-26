#include <security/pam_appl.h>
#include <pwd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static int converse(int count, const struct pam_message **messages,
                    struct pam_response **responses, void *data)
{
    (void)data;
    if (count != 1 || messages[0]->msg_style != PAM_PROMPT_ECHO_OFF)
        return PAM_CONV_ERR;
    *responses = calloc(1, sizeof(**responses));
    if (!*responses)
        return PAM_BUF_ERR;
    (*responses)->resp = strdup("fixture-password");
    return (*responses)->resp ? PAM_SUCCESS : PAM_BUF_ERR;
}

int main(int argc, char **argv)
{
    if (argc != 2)
        return 2;
    struct passwd *account = getpwuid(getuid());
    if (!account)
        return 2;
    struct pam_conv conversation = { .conv = converse, .appdata_ptr = NULL };
    for (int i = 0; i < 3; ++i) {
        pam_handle_t *pamh = NULL;
        int result = pam_start_confdir("real-faillock", account->pw_name,
                                       &conversation, argv[1], &pamh);
        if (result == PAM_SUCCESS)
            result = pam_authenticate(pamh, 0);
        if (pamh)
            pam_end(pamh, result);
        if (result != (i < 2 ? PAM_SUCCESS : PAM_AUTH_ERR)) {
            fprintf(stderr, "Попытка %d: неожиданный PAM=%d\n", i + 1, result);
            return 1;
        }
    }
    puts("Настоящий pam_faillock учитывает несовпадения и блокирует следующую проверку.");
    return 0;
}
