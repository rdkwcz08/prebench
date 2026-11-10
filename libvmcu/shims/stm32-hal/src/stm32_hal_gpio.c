/*
 * libvmcu/shims/stm32-hal/src/stm32_hal_gpio.c — HAL_GPIO_* et EXTI.
 *
 * Correspondance : GPIOx -> port = index (A = 0), GPIO_PIN_n -> pin = n.
 * Comme sur STM32, une ligne EXTI n'est reliée qu'à un seul port à la fois
 * (registre SYSCFG_EXTICR) : la dernière broche configurée en IT l'emporte.
 */
#include "stm32_hal_internal.h"

#include <stdatomic.h>

#define EXTI_LINES 16U
#define EXTI_NO_PORT 0xFFU

/* Bits du champ Mode (valeurs du HAL de ST). */
#define MODE_MASK      0x00000003U
#define MODE_OD        0x00000010U
#define MODE_EXTI_IT   0x00010000U
#define MODE_EXTI_EVT  0x00020000U
#define MODE_RISING    0x00100000U
#define MODE_FALLING   0x00200000U

static atomic_uchar exti_port[EXTI_LINES] = {
    EXTI_NO_PORT, EXTI_NO_PORT, EXTI_NO_PORT, EXTI_NO_PORT, EXTI_NO_PORT, EXTI_NO_PORT,
    EXTI_NO_PORT, EXTI_NO_PORT, EXTI_NO_PORT, EXTI_NO_PORT, EXTI_NO_PORT, EXTI_NO_PORT,
    EXTI_NO_PORT, EXTI_NO_PORT, EXTI_NO_PORT, EXTI_NO_PORT,
};
static atomic_uint exti_pending;

/* --- Utilitaires ------------------------------------------------------------------------------ */

static int port_index(const GPIO_TypeDef *gpio)
{
    if (gpio < &prebench_gpio_ports[0] || gpio >= &prebench_gpio_ports[PREBENCH_GPIO_PORT_COUNT]) {
        return -1;
    }
    return (int)(gpio - &prebench_gpio_ports[0]);
}

IRQn_Type prebench_exti_irqn(uint32_t line)
{
    switch (line) {
    case 0: return EXTI0_IRQn;
    case 1: return EXTI1_IRQn;
    case 2: return EXTI2_IRQn;
    case 3: return EXTI3_IRQn;
    case 4: return EXTI4_IRQn;
    default: return (line <= 9U) ? EXTI9_5_IRQn : EXTI15_10_IRQn;
    }
}

uint32_t prebench_exti_lines_of(IRQn_Type irqn)
{
    switch (irqn) {
    case EXTI0_IRQn:     return 0x0001U;
    case EXTI1_IRQn:     return 0x0002U;
    case EXTI2_IRQn:     return 0x0004U;
    case EXTI3_IRQn:     return 0x0008U;
    case EXTI4_IRQn:     return 0x0010U;
    case EXTI9_5_IRQn:   return 0x03E0U;
    case EXTI15_10_IRQn: return 0xFC00U;
    default:             return 0U;
    }
}

void prebench_exti_call_handler(IRQn_Type irqn)
{
    switch (irqn) {
    case EXTI0_IRQn:     EXTI0_IRQHandler(); break;
    case EXTI1_IRQn:     EXTI1_IRQHandler(); break;
    case EXTI2_IRQn:     EXTI2_IRQHandler(); break;
    case EXTI3_IRQn:     EXTI3_IRQHandler(); break;
    case EXTI4_IRQn:     EXTI4_IRQHandler(); break;
    case EXTI9_5_IRQn:   EXTI9_5_IRQHandler(); break;
    case EXTI15_10_IRQn: EXTI15_10_IRQHandler(); break;
    default: break;
    }
}

uint32_t prebench_exti_get_pending(uint32_t lines)
{
    return atomic_load(&exti_pending) & lines;
}

void prebench_exti_clear_pending(uint32_t lines)
{
    atomic_fetch_and(&exti_pending, ~lines);
}

/* Fil IRQ de libvmcu, verrou d'interruption tenu. */
static void on_edge(uint8_t port, uint8_t pin, vmcu_gpio_edge_t edge, void *user)
{
    (void)edge;
    (void)user;
    if (pin >= EXTI_LINES || atomic_load(&exti_port[pin]) != port) {
        return;
    }
    atomic_fetch_or(&exti_pending, 1U << pin);
    IRQn_Type irqn = prebench_exti_irqn(pin);
    if (prebench_nvic_enabled(irqn)) {
        prebench_exti_call_handler(irqn);
    }
}

static vmcu_gpio_mode_t to_vmcu_mode(uint32_t mode)
{
    bool od = (mode & MODE_OD) != 0U;
    switch (mode & MODE_MASK) {
    case 1U: return od ? VMCU_GPIO_MODE_OUTPUT_OD : VMCU_GPIO_MODE_OUTPUT_PP;
    case 2U: return od ? VMCU_GPIO_MODE_ALTERNATE_OD : VMCU_GPIO_MODE_ALTERNATE_PP;
    case 3U: return VMCU_GPIO_MODE_ANALOG;
    default: return VMCU_GPIO_MODE_INPUT;
    }
}

/* --- API HAL ---------------------------------------------------------------------------------- */

void HAL_GPIO_Init(GPIO_TypeDef *GPIOx, GPIO_InitTypeDef *GPIO_Init)
{
    int port = port_index(GPIOx);
    if (port < 0 || GPIO_Init == NULL) {
        vmcu_log(VMCU_LOG_ERROR, "HAL_GPIO_Init : port GPIO inconnu");
        return;
    }
    uint32_t mode = GPIO_Init->Mode;
    vmcu_gpio_mode_t vmode = to_vmcu_mode(mode);
    vmcu_gpio_pull_t pull = (GPIO_Init->Pull == GPIO_PULLUP)     ? VMCU_GPIO_PULL_UP
                          : (GPIO_Init->Pull == GPIO_PULLDOWN)   ? VMCU_GPIO_PULL_DOWN
                                                                 : VMCU_GPIO_PULL_NONE;
    bool alternate = (vmode == VMCU_GPIO_MODE_ALTERNATE_PP || vmode == VMCU_GPIO_MODE_ALTERNATE_OD);
    uint8_t af = alternate ? (uint8_t)(GPIO_Init->Alternate & 0x0FU) : VMCU_GPIO_AF_NONE;

    for (uint8_t pin = 0; pin < EXTI_LINES; ++pin) {
        if ((GPIO_Init->Pin & (1U << pin)) == 0U) {
            continue;
        }
        (void)vmcu_gpio_config((uint8_t)port, pin, vmode, pull, af);

        if ((mode & (MODE_EXTI_IT | MODE_EXTI_EVT)) != 0U) {
            atomic_store(&exti_port[pin], (unsigned char)port);
            vmcu_gpio_edge_t edge = (vmcu_gpio_edge_t)(((mode & MODE_RISING) ? VMCU_GPIO_EDGE_RISING : 0) |
                                                       ((mode & MODE_FALLING) ? VMCU_GPIO_EDGE_FALLING : 0));
            /* Mode « événement » (WFE) : pas d'interruption. */
            if ((mode & MODE_EXTI_IT) != 0U) {
                (void)vmcu_gpio_on_edge((uint8_t)port, pin, edge, on_edge, NULL);
            } else {
                (void)vmcu_gpio_on_edge((uint8_t)port, pin, VMCU_GPIO_EDGE_NONE, NULL, NULL);
            }
        } else if (atomic_load(&exti_port[pin]) == (unsigned char)port) {
            atomic_store(&exti_port[pin], (unsigned char)EXTI_NO_PORT);
            (void)vmcu_gpio_on_edge((uint8_t)port, pin, VMCU_GPIO_EDGE_NONE, NULL, NULL);
        }
    }
}

void HAL_GPIO_DeInit(GPIO_TypeDef *GPIOx, uint32_t GPIO_Pin)
{
    int port = port_index(GPIOx);
    if (port < 0) {
        return;
    }
    for (uint8_t pin = 0; pin < EXTI_LINES; ++pin) {
        if ((GPIO_Pin & (1U << pin)) == 0U) {
            continue;
        }
        if (atomic_load(&exti_port[pin]) == (unsigned char)port) {
            atomic_store(&exti_port[pin], (unsigned char)EXTI_NO_PORT);
            (void)vmcu_gpio_on_edge((uint8_t)port, pin, VMCU_GPIO_EDGE_NONE, NULL, NULL);
        }
        (void)vmcu_gpio_config((uint8_t)port, pin, VMCU_GPIO_MODE_DISABLED, VMCU_GPIO_PULL_NONE,
                               VMCU_GPIO_AF_NONE);
    }
}

GPIO_PinState HAL_GPIO_ReadPin(GPIO_TypeDef *GPIOx, uint16_t GPIO_Pin)
{
    int port = port_index(GPIOx);
    if (port < 0) {
        return GPIO_PIN_RESET;
    }
    /* Comme le HAL : SET si l'un des bits du masque est à 1. */
    for (uint8_t pin = 0; pin < EXTI_LINES; ++pin) {
        if ((GPIO_Pin & (1U << pin)) != 0U && vmcu_gpio_read((uint8_t)port, pin) == VMCU_HIGH) {
            return GPIO_PIN_SET;
        }
    }
    return GPIO_PIN_RESET;
}

void HAL_GPIO_WritePin(GPIO_TypeDef *GPIOx, uint16_t GPIO_Pin, GPIO_PinState PinState)
{
    int port = port_index(GPIOx);
    if (port < 0) {
        return;
    }
    vmcu_level_t level = (PinState != GPIO_PIN_RESET) ? VMCU_HIGH : VMCU_LOW;
    for (uint8_t pin = 0; pin < EXTI_LINES; ++pin) {
        if ((GPIO_Pin & (1U << pin)) != 0U) {
            (void)vmcu_gpio_write((uint8_t)port, pin, level);
        }
    }
}

void HAL_GPIO_TogglePin(GPIO_TypeDef *GPIOx, uint16_t GPIO_Pin)
{
    int port = port_index(GPIOx);
    if (port < 0) {
        return;
    }
    for (uint8_t pin = 0; pin < EXTI_LINES; ++pin) {
        if ((GPIO_Pin & (1U << pin)) != 0U) {
            vmcu_level_t now = vmcu_gpio_read((uint8_t)port, pin);
            (void)vmcu_gpio_write((uint8_t)port, pin, (now == VMCU_HIGH) ? VMCU_LOW : VMCU_HIGH);
        }
    }
}

HAL_StatusTypeDef HAL_GPIO_LockPin(GPIO_TypeDef *GPIOx, uint16_t GPIO_Pin)
{
    UNUSED(GPIOx);
    UNUSED(GPIO_Pin);
    vmcu_warn_unsimulated("HAL_GPIO_LockPin");
    return HAL_OK;
}

void HAL_GPIO_EXTI_IRQHandler(uint16_t GPIO_Pin)
{
    if (prebench_exti_get_pending(GPIO_Pin) != 0U) {
        prebench_exti_clear_pending(GPIO_Pin);
        HAL_GPIO_EXTI_Callback(GPIO_Pin);
    }
}
