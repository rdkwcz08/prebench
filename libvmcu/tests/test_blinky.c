/*
 * libvmcu/tests/test_blinky.c — Modèle templates/blinky-stm32-hal face à une fausse app.
 *
 * Sources du modèle compilées telles quelles (main renommé). Vérifie, d'après
 * l'horodatage des trames GPIO_WRITE de LD2 : période de 500 ms, puis 100 ms
 * après un appui sur B1 (HAL_GPIO_EXTI_Callback du modèle).
 */
#include <stdio.h>
#include <stdlib.h>

#include "fake_app.h"
#include "vmcu_platform.h"
#include "vmcu_wire.h"

int prebench_firmware_main(void);

static void firmware_thread(void *arg)
{
    (void)arg;
    (void)prebench_firmware_main();
}

/* Intervalle moyen (ms) entre les écritures n et n + k de LD2 (PA5). */
static double mean_period_ms(fake_app_t *fa, unsigned n, unsigned k)
{
    fake_frame_t a, b;
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_WRITE, 0, 5, n, 10000, &a));
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_WRITE, 0, 5, n + k, 10000, &b));
    return (double)(b.timestamp_us - a.timestamp_us) / 1000.0 / (double)k;
}

int main(void)
{
    fake_app_t *fa = fake_app_create(1);
    fake_app_initial_input(fa, 2, 13, 1);
    fake_app_run(fa);
    CHECK(vmcu_plat_thread_start(firmware_thread, NULL) != NULL);

    /* Écriture 1 : mise à 0 dans MX_GPIO_Init ; puis bascules toutes les 500 ms. */
    double slow = mean_period_ms(fa, 2, 3);
    printf("période lente : %.1f ms\n", slow);
    CHECK(slow > 450.0 && slow < 650.0);

    /* Appui sur B1 -> 100 ms */
    fake_app_send_gpio_input(fa, 2, 13, 0);
    vmcu_plat_sleep_ns(20000000ull);
    fake_app_send_gpio_input(fa, 2, 13, 1);
    unsigned now = fake_app_count(fa, VMCU_MSG_GPIO_WRITE, 0, 5);
    double fast = mean_period_ms(fa, now + 2, 5); /* laisse passer le délai de 500 ms en cours */
    printf("période rapide : %.1f ms\n", fast);
    CHECK(fast > 90.0 && fast < 200.0);

    printf("test_blinky : OK\n");
    fflush(stdout);
    _Exit(0);
}
