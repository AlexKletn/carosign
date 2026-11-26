#include <security/pam_appl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

struct fixture { const char *password; int prompts; };

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
    if (!answer->resp) { free(answer); return PAM_BUF_ERR; }
    fixture->prompts++;
    *responses = answer;
    return PAM_SUCCESS;
}

static int run(const char *directory, const char *service, const char *password,
               int expected, int prompts, const char *expected_trace)
{
    const char *trace = getenv("CAROSIGN_TEST_PAM_TRACE");
    if (!trace || (unlink(trace) != 0 && access(trace, F_OK) == 0))
        return 1;
    struct fixture fixture = { .password = password, .prompts = 0 };
    struct pam_conv conversation = { .conv = converse, .appdata_ptr = &fixture };
    pam_handle_t *pamh = NULL;
    int result = pam_start_confdir(service, "fixture-user", &conversation, directory, &pamh);
    if (result == PAM_SUCCESS)
        result = pam_authenticate(pamh, 0);
    int end = pamh ? pam_end(pamh, result) : PAM_SYSTEM_ERR;
    FILE *input = fopen(trace, "r");
    char content[512] = {0};
    if (!input)
        return 1;
    size_t length = fread(content, 1, sizeof(content) - 1, input);
    int io_error = ferror(input) || !feof(input);
    fclose(input);
    content[length] = '\0';
    if (result != expected || fixture.prompts != prompts || end != PAM_SUCCESS ||
        io_error || strcmp(content, expected_trace) != 0) {
        fprintf(stderr, "%s: PAM=%d prompts=%d end=%d trace=%s", service,
                result, fixture.prompts, end, content);
        return 1;
    }
    return 0;
}

int main(int argc, char **argv)
{
    if (argc != 2)
        return 2;
    if (run(argv[1], "matched", "unused", PAM_SUCCESS, 0, "preauth\nbio-match\nauthsucc\nenv\n") ||
        run(argv[1], "unavailable", "fixture-password", PAM_SUCCESS, 1, "preauth\nbio-unavailable\n") ||
        run(argv[1], "no-match", "fixture-password", PAM_SUCCESS, 1, "preauth\nbio-no-match\nauthfail\n") ||
        run(argv[1], "unavailable", "wrong", PAM_AUTH_ERR, 1, "preauth\nbio-unavailable\n") ||
        run(argv[1], "locked", "unused", PAM_AUTH_ERR, 0, "locked\n"))
        return 1;
    puts("Изолированный libpam: biometrics, пароль, отказ и preauth проверены.");
    return 0;
}
