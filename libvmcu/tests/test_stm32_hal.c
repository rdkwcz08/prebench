/*
 * libvmcu/tests/test_stm32_hal.c — Shim STM32 HAL face à une fausse app.
 *
 * Écrit comme un firmware CubeMX : HAL_Init, MX_GPIO_Init (LD2 sur PA5, B1 sur
 * PC13 en interruption descendante), HAL_GPIO_EXTI_Callback défini par le
 * projet, EXTI15_10_IRQHandler fourni par le shim (non défini ici).
 */
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>

#include "fake_app.h"
#include "stm32f4xx_hal.h"
#include "vmcu_platform.h"
#include "vmcu_wire.h"

#define LD2_Pin       GPIO_PIN_5
#define LD2_GPIO_Port GPIOA
#define B1_Pin        GPIO_PIN_13
#define B1_GPIO_Port  GPIOC

static atomic_int callbacks;
static atomic_int last_pin;

/* Défini par le projet : remplace celui du shim. */
void HAL_GPIO_EXTI_Callback(uint16_t GPIO_Pin)
{
    atomic_store(&last_pin, GPIO_Pin);
    atomic_fetch_add(&callbacks, 1);
}

static void MX_GPIO_Init(void)
{
    GPIO_InitTypeDef GPIO_InitStruct = {0};
    __HAL_RCC_GPIOC_CLK_ENABLE();
    __HAL_RCC_GPIOA_CLK_ENABLE();
    HAL_GPIO_WritePin(LD2_GPIO_Port, LD2_Pin, GPIO_PIN_RESET);

    GPIO_InitStruct.Pin = B1_Pin;
    GPIO_InitStruct.Mode = GPIO_MODE_IT_FALLING;
    GPIO_InitStruct.Pull = GPIO_NOPULL;
    HAL_GPIO_Init(B1_GPIO_Port, &GPIO_InitStruct);

    GPIO_InitStruct.Pin = LD2_Pin;
    GPIO_InitStruct.Mode = GPIO_MODE_OUTPUT_PP;
    GPIO_InitStruct.Pull = GPIO_NOPULL;
    GPIO_InitStruct.Speed = GPIO_SPEED_FREQ_LOW;
    HAL_GPIO_Init(LD2_GPIO_Port, &GPIO_InitStruct);

    HAL_NVIC_SetPriority(EXTI15_10_IRQn, 0, 0);
    HAL_NVIC_EnableIRQ(EXTI15_10_IRQn);
}

static bool wait_callbacks(int expected, uint32_t timeout_ms)
{
    for (uint32_t t = 0; t < timeout_ms; t += 5) {
        if (atomic_load(&callbacks) >= expected) {
            return true;
        }
        vmcu_plat_sleep_ns(5000000ull);
    }
    return atomic_load(&callbacks) >= expected;
}

/* Appui sur B1 : front descendant (le relâchement remonte le niveau). */
static void press_b1(fake_app_t *fa)
{
    fake_app_send_gpio_input(fa, 2, 13, 0);
    vmcu_plat_sleep_ns(20000000ull);
    fake_app_send_gpio_input(fa, 2, 13, 1);
}

int main(void)
{
    fake_app_t *fa = fake_app_create(1);
    fake_app_initial_input(fa, 2, 13, 1);
    fake_app_run(fa);

    CHECK(HAL_Init() == HAL_OK);
    MX_GPIO_Init();

    /* B1 : entrée, interruption sur front descendant */
    fake_frame_t f;
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 2, 13, 2, 1000, &f));
    CHECK(f.data[2] == VMCU_GPIO_MODE_INPUT && f.data[4] == VMCU_GPIO_EDGE_FALLING);

    /* LD2 : écriture, bascule, relecture */
    HAL_GPIO_WritePin(LD2_GPIO_Port, LD2_Pin, GPIO_PIN_SET);
    CHECK(HAL_GPIO_ReadPin(LD2_GPIO_Port, LD2_Pin) == GPIO_PIN_SET);
    HAL_GPIO_TogglePin(LD2_GPIO_Port, LD2_Pin);
    CHECK(HAL_GPIO_ReadPin(LD2_GPIO_Port, LD2_Pin) == GPIO_PIN_RESET);
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_WRITE, 0, 5, 3, 500, &f)); /* RESET, SET, RESET */
    CHECK(f.data[2] == 0);

    /* Appui sur B1 -> HAL_GPIO_EXTI_Callback(GPIO_PIN_13) */
    CHECK(HAL_GPIO_ReadPin(B1_GPIO_Port, B1_Pin) == GPIO_PIN_SET);
    press_b1(fa);
    CHECK(wait_callbacks(1, 2000));
    CHECK(atomic_load(&last_pin) == B1_Pin);
    vmcu_plat_sleep_ns(50000000ull);
    CHECK(atomic_load(&callbacks) == 1); /* le relâchement ne déclenche rien */

    /* NVIC désactivé : le front reste en attente, servi à la réactivation */
    HAL_NVIC_DisableIRQ(EXTI15_10_IRQn);
    press_b1(fa);
    vmcu_plat_sleep_ns(100000000ull);
    CHECK(atomic_load(&callbacks) == 1);
    HAL_NVIC_EnableIRQ(EXTI15_10_IRQn);
    CHECK(wait_callbacks(2, 2000));

    /* __disable_irq retarde le callback jusqu'à __enable_irq */
    __disable_irq();
    press_b1(fa);
    vmcu_plat_sleep_ns(100000000ull);
    CHECK(atomic_load(&callbacks) == 2);
    __enable_irq();
    CHECK(wait_callbacks(3, 2000));

    /* Tick et délai */
    uint32_t t0 = HAL_GetTick();
    HAL_Delay(10);
    uint32_t dt = HAL_GetTick() - t0;
    CHECK(dt >= 10u && dt < 500u);

    /* Fonction non simulée : compile, avertit une fois */
    CHECK(HAL_GPIO_LockPin(LD2_GPIO_Port, LD2_Pin) == HAL_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_WARN, -1, -1, 1, 500, &f));

    vmcu_shutdown();
    fake_app_stop(fa);
    printf("test_stm32_hal : OK\n");
    return 0;
}
