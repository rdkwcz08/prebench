/*
 * libvmcu/tests/test_cubemx.c — Projet STM32CubeMX de référence (tests/cubemx-nucleo-f401re/ :
 * configuration par défaut + I2C1, code ajouté dans les seules sections USER CODE).
 *
 * Les sources du projet sont compilées telles quelles ; seul `main` est
 * renommé (-Dmain=prebench_firmware_main) pour que ce test l'exécute dans un
 * fil face à une fausse app. Vérifie que l'initialisation CubeMX se déroule
 * sans Error_Handler : LD2, B1 (interruption), USART2 via HAL_UART_MspInit
 * (simulée), I2C1 via HAL_I2C_MspInit (simulé), puis la lecture
 * HAL_I2C_Mem_Read_IT d'un esclave en 0x50 et son callback, affichée sur USART2.
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
    fake_app_set_i2c_device(fa, 0x50);
    fake_app_set_i2c_reg(fa, 0x00, 0x2A);
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
    /* USART2 simulée : débit annoncé (115 200 bauds), sans avertissement « non simulé » */
    CHECK(fake_app_wait(fa, VMCU_MSG_UART_CONFIG, 2, -1, 1, 3000, &f));
    CHECK(f.data[4] == 0x00 && f.data[5] == 0xC2 && f.data[6] == 0x01);
    CHECK(!warned(fa, "HAL_UART_Init"));

    /* I2C1 (HAL_I2C_MspInit du projet) : PB8 SCL et PB9 SDA en AF4 drain ouvert, 100 kHz */
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 1, 8, 1, 3000, &f));
    CHECK(f.data[2] == VMCU_GPIO_MODE_ALTERNATE_OD && f.data[5] == 4);
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 1, 9, 1, 3000, &f));
    CHECK(f.data[2] == VMCU_GPIO_MODE_ALTERNATE_OD && f.data[5] == 4);
    CHECK(fake_app_wait(fa, VMCU_MSG_I2C_CONFIG, 1, -1, 1, 3000, &f));
    CHECK(vmcu_wire_get_u32(f.data + 4) == 100000u);
    CHECK(!warned(fa, "HAL_I2C_Init"));

    /* USER CODE : Mem_Read_IT du registre 0x00, callback, ligne sur USART2 */
    const char *expected = "I2C 0x50 reg 0x00 = 0x2A\r\n";
    CHECK(fake_app_wait(fa, VMCU_MSG_UART_TX, 2, -1, 1, 3000, &f));
    CHECK(f.data[2] == strlen(expected) && memcmp(f.data + 4, expected, strlen(expected)) == 0);

    /* Appui sur B1 : traité par EXTI15_10_IRQHandler du projet, sans plantage */
    fake_app_send_gpio_input(fa, 2, 13, 0);
    vmcu_plat_sleep_ns(50000000ull);

    printf("test_cubemx : OK\n");
    fflush(stdout);
    /* Le firmware reste dans sa boucle infinie : on termine le processus. */
    _Exit(0);
}
