#include <security/pam_appl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct fixture {
    const char *password;
    int prompts;
};

static int converse(int count, const struct pam_message **messages,
                    struct pam_response **responses, void *data)
{
    struct fixture *fixture = data;
    if (count != 1 || messages[0]->msg_style != PAM_PROMPT_ECHO_OFF)
        return PAM_CONV_ERR;
    struct pam_response *answer = calloc(1, sizeof(*answer));
    if (!answer)
        return PAM_BUF_ERR;
    answer->resp = strdup(fixture->password);
    if (!answer->resp) {
        free(answer);
        return PAM_BUF_ERR;
    }
    fixture->prompts++;
    *responses = answer;
    return PAM_SUCCESS;
}

static int run(const char *directory, const char *service, const char *password,
               int expected, int prompts)
{
    struct fixture fixture = { .password = password, .prompts = 0 };
    struct pam_conv conversation = { .conv = converse, .appdata_ptr = &fixture };
    pam_handle_t *pamh = NULL;
    int result = pam_start_confdir(service, "fixture-user", &conversation, directory, &pamh);
    if (result != PAM_SUCCESS) {
        fprintf(stderr, "Не удалось открыть изолированный стек PAM: %d\n", result);
        return 1;
    }
    result = pam_authenticate(pamh, 0);
    int end_result = pam_end(pamh, result);
    if (result != expected || fixture.prompts != prompts || end_result != PAM_SUCCESS) {
        fprintf(stderr, "Неожиданный результат PAM: %d, запросов: %d, закрытие: %d\n",
                result, fixture.prompts, end_result);
        return 1;
    }
    return 0;
}

int main(int argc, char **argv)
{
    if (argc != 2)
        return 2;
    if (run(argv[1], "carosign-only", "unused", PAM_AUTHINFO_UNAVAIL, 0) ||
        run(argv[1], "password", "fixture-password", PAM_SUCCESS, 1) ||
        run(argv[1], "password", "wrong-password", PAM_AUTH_ERR, 1) ||
        run(argv[1], "token", "unused", PAM_SUCCESS, 0))
        return 1;
    puts("Изолированный стек libpam: fallback и сохранение PAM_AUTHTOK проверены.");
    return 0;
}
