/*
 * libvmcu/core/vmcu_wire.h — Encodage des trames du protocole v1 (docs/protocol.md).
 *
 * Tout est lu et écrit octet par octet, en little-endian : aucune copie de
 * struct C sur le fil, donc aucun problème d'alignement ni de remplissage.
 * Module sans état ni appel système : utilisable par libvmcu et par les tests.
 */
#ifndef VMCU_WIRE_H
#define VMCU_WIRE_H

#include <stddef.h>
#include <stdint.h>

#define VMCU_WIRE_MAGIC        0x4250u /* 'P' 'B' sur le fil */
#define VMCU_WIRE_HEADER_SIZE  24u
#define VMCU_WIRE_MAX_PAYLOAD  65536u
#define VMCU_WIRE_MAX_STR      1024u

#define VMCU_FLAG_SYNC  0x0001u
#define VMCU_FLAG_REPLY 0x0002u

/* Firmware -> app (section 4.1) */
#define VMCU_MSG_HELLO          0x0001u
#define VMCU_MSG_GPIO_CONFIG    0x0010u
#define VMCU_MSG_GPIO_WRITE     0x0011u
#define VMCU_MSG_PWM_CONFIG     0x0020u
#define VMCU_MSG_PWM_DUTY       0x0021u
#define VMCU_MSG_PWM_ATTACH     0x0022u
#define VMCU_MSG_ADC_CONFIG     0x0030u
#define VMCU_MSG_UART_CONFIG    0x0040u
#define VMCU_MSG_UART_TX        0x0041u
#define VMCU_MSG_I2C_CONFIG     0x0050u
#define VMCU_MSG_I2C_XFER       0x0051u
#define VMCU_MSG_SPI_CONFIG     0x0060u
#define VMCU_MSG_SPI_XFER       0x0061u
#define VMCU_MSG_STREAM_OPEN    0x0070u
#define VMCU_MSG_STREAM_CLOSE   0x0071u
#define VMCU_MSG_STREAM_NOTIFY  0x0072u
#define VMCU_MSG_WARN           0x0080u
#define VMCU_MSG_LOG            0x0081u
#define VMCU_MSG_HEARTBEAT      0x0090u /* protocole 1.1 : mode débogage */
#define VMCU_MSG_PING           0x00F0u

/* App -> firmware (section 4.2) */
#define VMCU_MSG_WELCOME        0x8001u
#define VMCU_MSG_REJECT         0x8002u
#define VMCU_MSG_GPIO_INPUT     0x8010u
#define VMCU_MSG_ADC_VALUE      0x8030u
#define VMCU_MSG_UART_RX        0x8041u
#define VMCU_MSG_I2C_REPLY      0x8051u
#define VMCU_MSG_SPI_REPLY      0x8061u
#define VMCU_MSG_STREAM_OPENED  0x8070u
#define VMCU_MSG_STREAM_NOTIFY_APP 0x8072u
#define VMCU_MSG_RESET          0x80E0u
#define VMCU_MSG_PAUSE          0x80E1u
#define VMCU_MSG_RESUME         0x80E2u
#define VMCU_MSG_SHUTDOWN       0x80E3u
#define VMCU_MSG_PONG           0x80F0u

/* Codes de WARN (section 4.1) */
#define VMCU_WARN_GENERIC      0u
#define VMCU_WARN_UNSIMULATED  1u
#define VMCU_WARN_UART_OVERRUN 2u
#define VMCU_WARN_BUSY_LOOP    3u
#define VMCU_WARN_NO_RESOURCE  4u

typedef struct {
    uint16_t type;
    uint16_t board_id;
    uint16_t flags;
    uint32_t seq;
    uint32_t length;
    uint64_t timestamp_us;
} vmcu_frame_header_t;

typedef enum {
    VMCU_WIRE_OK        =  0,
    VMCU_WIRE_BAD_MAGIC = -1,
    VMCU_WIRE_TOO_LONG  = -2
} vmcu_wire_result_t;

void     vmcu_wire_put_u16(uint8_t *p, uint16_t v);
void     vmcu_wire_put_u32(uint8_t *p, uint32_t v);
void     vmcu_wire_put_u64(uint8_t *p, uint64_t v);
uint16_t vmcu_wire_get_u16(const uint8_t *p);
uint32_t vmcu_wire_get_u32(const uint8_t *p);
uint64_t vmcu_wire_get_u64(const uint8_t *p);

void vmcu_wire_encode_header(uint8_t out[VMCU_WIRE_HEADER_SIZE], const vmcu_frame_header_t *h);
vmcu_wire_result_t vmcu_wire_decode_header(const uint8_t in[VMCU_WIRE_HEADER_SIZE],
                                           vmcu_frame_header_t *h);

/* Écriture séquentielle d'une charge utile dans un tampon borné. */
typedef struct {
    uint8_t *buf;
    size_t   cap;
    size_t   len;
    int      overflow; /* 1 si une écriture a été refusée faute de place */
} vmcu_wbuf_t;

void vmcu_wbuf_init(vmcu_wbuf_t *w, uint8_t *buf, size_t cap);
void vmcu_wbuf_u8(vmcu_wbuf_t *w, uint8_t v);
void vmcu_wbuf_u16(vmcu_wbuf_t *w, uint16_t v);
void vmcu_wbuf_u32(vmcu_wbuf_t *w, uint32_t v);
void vmcu_wbuf_u64(vmcu_wbuf_t *w, uint64_t v);
void vmcu_wbuf_bytes(vmcu_wbuf_t *w, const void *data, size_t len);
/* str : u16 longueur + UTF-8, tronquée à VMCU_WIRE_MAX_STR octets. NULL = chaîne vide. */
void vmcu_wbuf_str(vmcu_wbuf_t *w, const char *s);

/* Lecture séquentielle d'une charge utile. Lire au-delà de la fin lève `error`
 * et renvoie 0 : les champs ajoutés en fin de charge utile par une version
 * mineure plus récente sont simplement ignorés. */
typedef struct {
    const uint8_t *p;
    size_t         len;
    size_t         pos;
    int            error;
} vmcu_rbuf_t;

void     vmcu_rbuf_init(vmcu_rbuf_t *r, const uint8_t *p, size_t len);
uint8_t  vmcu_rbuf_u8(vmcu_rbuf_t *r);
uint16_t vmcu_rbuf_u16(vmcu_rbuf_t *r);
uint32_t vmcu_rbuf_u32(vmcu_rbuf_t *r);
uint64_t vmcu_rbuf_u64(vmcu_rbuf_t *r);
/* Copie une str dans out (terminée par '\0', tronquée à cap - 1). */
void     vmcu_rbuf_str(vmcu_rbuf_t *r, char *out, size_t cap);

#endif /* VMCU_WIRE_H */
