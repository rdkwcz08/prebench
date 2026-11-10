/*
 * libvmcu/tests/test_cubemx.c — Projet STM32CubeMX non modifié (tests/cubemx-nucleo-f401re/).
 *
 * Les sources du projet sont compilées telles quelles ; seul `main` est
 * renommé (-Dmain=prebench_firmware_main) pour que ce test l'exécute dans un
 * fil face à une fausse app. Vérifie que l'initialisation CubeMX se déroule
 * sans Error_Handler : LD2, B1 (interruption), USART2 via HAL_UART_MspInit,
 * avertissement « HAL_UART_Init non simulé ».
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "fake_app.h"
#include "vmcu.h"
#include "vmcu_platform.h"
#include "vmcu_wire.h"

int prebench_firmware_main(void);

static void firmware_thread(void *arg)
{
    (void)arg;
    (void)prebench_firmware_main(); /* boucle infinie du projet */
}

static bool warned(fake_app_t *fa, const char *name)
{
    fake_frame_t f;
    size_t n = strlen(name);
    for (unsigned count = 1; fake_app_wait(fa, VMCU_MSG_WARN, -1, -1, count, 1000, &f); ++count) {
        if (f.data[0] == VMCU_WARN_UNSIMULATED && vmcu_wire_get_u16(f.data + 2) == n &&
            memcmp(f.data + 4, name, n) == 0) {
            return true;
        }
    }
    return false;
}

int main(void)
{
    fake_app_t *fa = fake_app_create(1);
    fake_app_initial_input(fa, 2, 13, 1);
    fake_app_run(fa);
    CHECK(vmcu_plat_thread_start(firmware_thread, NULL) != NULL);

    fake_frame_t f;
    /* B1 (PC13) : entrée, interruption sur front descendant */
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 2, 13, 2, 3000, &f));
    CHECK(f.data[2] == VMCU_GPIO_MODE_INPUT && f.data[4] == VMCU_GPIO_EDGE_FALLING);
    /* LD2 (PA5) : sortie push-pull, mise à 0 */
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 0, 5, 1, 3000, &f));
    CHECK(f.data[2] == VMCU_GPIO_MODE_OUTPUT_PP);
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_WRITE, 0, 5, 1, 3000, &f));
    CHECK(f.data[2] == 0);
    /* USART2 : PA2 et PA3 en fonction alternative AF7 (HAL_UART_MspInit du projet) */
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 0, 2, 1, 3000, &f));
    CHECK(f.data[2] == VMCU_GPIO_MODE_ALTERNATE_PP && f.data[5] == 7);
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 0, 3, 1, 3000, &f));
    CHECK(f.data[2] == VMCU_GPIO_MODE_ALTERNATE_PP && f.data[5] == 7);
    CHECK(warned(fa, "HAL_UART_Init"));

    /* Appui sur B1 : traité par EXTI15_10_IRQHandler du projet, sans plantage */
    fake_app_send_gpio_input(fa, 2, 13, 0);
    vmcu_plat_sleep_ns(50000000ull);

    printf("test_cubemx : OK\n");
    fflush(stdout);
    /* Le firmware reste dans sa boucle infinie : on termine le processus. */
    _Exit(0);
}
