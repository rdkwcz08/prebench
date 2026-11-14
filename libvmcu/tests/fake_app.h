/*
 * libvmcu/tests/fake_app.h — Fausse app pour les tests : serveur du protocole v1
 * dans un fil du processus de test (le vrai serveur est le cœur Rust).
 */
#ifndef FAKE_APP_H
#define FAKE_APP_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct {
    unsigned index;      /* ordre d'arrivée, à partir de 0 */
    uint16_t type;
    uint16_t flags;
    uint32_t seq;
    uint32_t length;
    uint64_t timestamp_us; /* horodatage firmware (vmcu_micros) */
    uint8_t  data[64];   /* début de la charge utile */
} fake_frame_t;

typedef struct fake_app fake_app_t;

/** Crée la fausse app (rien n'est encore lancé). */
fake_app_t *fake_app_create(uint16_t board_id);

/** État initial d'une entrée, envoyé entre HELLO et WELCOME. */
void fake_app_initial_input(fake_app_t *fa, uint8_t port, uint8_t pin, uint8_t level);

/** Valeur ADC initiale, envoyée entre HELLO et WELCOME (comme le cœur). */
void fake_app_initial_adc(fake_app_t *fa, uint8_t unit, uint8_t channel, uint16_t raw);

/** Écoute, pose PREBENCH_SOCKET / PREBENCH_BOARD_ID, lance le fil serveur. */
void fake_app_run(fake_app_t *fa);

/** Présence d'un composant d'écho (PING cible Worker). */
void fake_app_set_echo(fake_app_t *fa, bool present);

void fake_app_send_gpio_input(fake_app_t *fa, uint8_t port, uint8_t pin, uint8_t level);
void fake_app_send_empty(fake_app_t *fa, uint16_t type); /* PAUSE, RESUME… */
/** Valeur brute d'un canal ADC (ADC_VALUE). */
void fake_app_send_adc(fake_app_t *fa, uint8_t unit, uint8_t channel, uint16_t raw);
/** Octets vers une UART du firmware (UART_RX), en trames de 60 octets au plus. */
void fake_app_send_uart_rx(fake_app_t *fa, uint8_t id, const uint8_t *data, size_t len);

/**
 * Attend qu'au moins `count` trames du type donné soient arrivées (port et pin
 * comparés aux deux premiers octets si >= 0). Renvoie la dernière dans *out.
 */
bool fake_app_wait(fake_app_t *fa, uint16_t type, int port, int pin, unsigned count,
                   uint32_t timeout_ms, fake_frame_t *out);

/** Nombre de trames reçues du type donné (filtre port/pin comme fake_app_wait). */
unsigned fake_app_count(fake_app_t *fa, uint16_t type, int port, int pin);

/** Chaînes reçues dans HELLO. */
const char *fake_app_hello_board(fake_app_t *fa);
const char *fake_app_hello_framework(fake_app_t *fa);

/** Attend la fermeture côté firmware, puis libère. */
void fake_app_stop(fake_app_t *fa);

/* Vérification minimale pour les tests. */
#define CHECK(cond)                                                                 \
    do {                                                                            \
        if (!(cond)) {                                                              \
            fprintf(stderr, "ÉCHEC %s:%d : %s\n", __FILE__, __LINE__, #cond);       \
            exit(1);                                                                \
        }                                                                           \
    } while (0)

#endif /* FAKE_APP_H */
