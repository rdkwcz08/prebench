/*
 * libvmcu/tests/test_uart.c — UART du HAL STM32 sur libvmcu, face à une fausse app :
 * débit annoncé, émission (bloquante et _IT avec HAL_UART_TxCpltCallback), réception
 * bloquante et délai, réception _IT relancée dans le callback, ReceiveToIdle_IT,
 * débordement du tampon de réception (WARN code 2).
 */
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "fake_app.h"
#include "stm32f4xx_hal.h"
#include "vmcu_platform.h"
#include "vmcu_wire.h"

static UART_HandleTypeDef huart2;
static uint8_t rx_it[4];
static char lines[64];
static atomic_int rx_done;
static atomic_int tx_done;
static atomic_int idle_size;

void HAL_UART_RxCpltCallback(UART_HandleTypeDef *huart)
{
    if (huart == &huart2) {
        int n = atomic_fetch_add(&rx_done, 1);
        if (n < 15) {
            memcpy(lines + 4 * n, rx_it, 4);
        }
        if (n == 0) {
            (void)HAL_UART_Receive_IT(&huart2, rx_it, 4); /* relancée dans le callback */
        }
    }
}

void HAL_UART_TxCpltCallback(UART_HandleTypeDef *huart)
{
    if (huart == &huart2) {
        atomic_fetch_add(&tx_done, 1);
    }
}

void HAL_UARTEx_RxEventCallback(UART_HandleTypeDef *huart, uint16_t Size)
{
    if (huart == &huart2) {
        atomic_store(&idle_size, (int)Size);
    }
}

static bool wait_for(atomic_int *v, int expected)
{
    for (int t = 0; t < 200; ++t) {
        if (atomic_load(v) >= expected) {
            return true;
        }
        vmcu_plat_sleep_ns(5000000ull);
    }
    return false;
}

int main(void)
{
    fake_app_t *fa = fake_app_create(1);
    fake_app_run(fa);
    CHECK(HAL_Init() == HAL_OK);

    /* Débit annoncé : UART_CONFIG id 2, 9 600 bauds. */
    huart2.Instance = USART2;
    huart2.Init.BaudRate = 9600;
    huart2.Init.Mode = UART_MODE_TX_RX;
    CHECK(HAL_UART_Init(&huart2) == HAL_OK);
    fake_frame_t f;
    CHECK(fake_app_wait(fa, VMCU_MSG_UART_CONFIG, 2, -1, 1, 1000, &f));
    CHECK(f.data[4] == 0x80 && f.data[5] == 0x25 && f.data[6] == 0 && f.data[7] == 0);

    /* Émission bloquante : UART_TX avec les octets. */
    const char hello[] = "Bonjour\r\n";
    CHECK(HAL_UART_Transmit(&huart2, (const uint8_t *)hello, (uint16_t)strlen(hello), 100) == HAL_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_UART_TX, 2, -1, 1, 1000, &f));
    CHECK(f.data[2] == strlen(hello) && memcmp(f.data + 4, hello, strlen(hello)) == 0);

    /* Émission _IT : octets envoyés, puis HAL_UART_TxCpltCallback. */
    CHECK(HAL_UART_Transmit_IT(&huart2, (const uint8_t *)"ok", 2) == HAL_OK);
    CHECK(wait_for(&tx_done, 1));
    CHECK(huart2.gState == HAL_UART_STATE_READY);

    /* Réception bloquante : délai dépassé, puis octets reçus. */
    uint8_t buf[8] = { 0 };
    CHECK(HAL_UART_Receive(&huart2, buf, 1, 5) == HAL_TIMEOUT);
    fake_app_send_uart_rx(fa, 2, (const uint8_t *)"abc", 3);
    CHECK(HAL_UART_Receive(&huart2, buf, 3, 1000) == HAL_OK);
    CHECK(memcmp(buf, "abc", 3) == 0);

    /* Réception _IT de 4 octets, relancée dans le callback : deux paquets. */
    CHECK(HAL_UART_Receive_IT(&huart2, rx_it, 4) == HAL_OK);
    fake_app_send_uart_rx(fa, 2, (const uint8_t *)"12", 2);
    vmcu_plat_sleep_ns(20000000ull);
    CHECK(atomic_load(&rx_done) == 0);
    fake_app_send_uart_rx(fa, 2, (const uint8_t *)"34ABCD", 6);
    CHECK(wait_for(&rx_done, 2));
    CHECK(memcmp(lines, "1234ABCD", 8) == 0);

    /* ReceiveToIdle_IT : un paquet de 5 octets, tampon de 16. */
    uint8_t idle_buf[16];
    CHECK(HAL_UARTEx_ReceiveToIdle_IT(&huart2, idle_buf, sizeof idle_buf) == HAL_OK);
    fake_app_send_uart_rx(fa, 2, (const uint8_t *)"hello", 5);
    CHECK(wait_for(&idle_size, 5));
    CHECK(memcmp(idle_buf, "hello", 5) == 0);
    CHECK(HAL_UARTEx_GetRxEventType(&huart2) == HAL_UART_RXEVENT_IDLE);

    /* Débordement : 5 000 octets sans lecture, tampon de 4 096 -> WARN code 2. */
    static uint8_t flood[5000];
    memset(flood, 'x', sizeof flood);
    fake_app_send_uart_rx(fa, 2, flood, sizeof flood);
    CHECK(fake_app_wait(fa, VMCU_MSG_WARN, 2, 0, 1, 2000, &f)); /* code 2 (u16 LE) */
    CHECK(vmcu_uart_available(2) == VMCU_UART_RX_BUFFER_SIZE);

    vmcu_shutdown();
    fake_app_stop(fa);
    printf("test_uart : OK\n");
    return 0;
}
