/*
 * libvmcu/tests/test_wire.c — Encodage des trames (docs/protocol.md, section 2).
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "fake_app.h"
#include "vmcu_wire.h"

static void test_header_bytes(void)
{
    vmcu_frame_header_t h = {
        .type = 0x0011u,
        .board_id = 0x0102u,
        .flags = VMCU_FLAG_SYNC,
        .seq = 0x0A0B0C0Du,
        .length = 3u,
        .timestamp_us = 0x1122334455667788ull,
    };
    uint8_t raw[VMCU_WIRE_HEADER_SIZE];
    vmcu_wire_encode_header(raw, &h);
    const uint8_t expected[VMCU_WIRE_HEADER_SIZE] = {
        'P', 'B',                   /* magic 0x4250, little-endian */
        0x11, 0x00,                 /* type */
        0x02, 0x01,                 /* board_id */
        0x01, 0x00,                 /* flags */
        0x0D, 0x0C, 0x0B, 0x0A,     /* seq */
        0x03, 0x00, 0x00, 0x00,     /* length */
        0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, /* timestamp */
    };
    CHECK(memcmp(raw, expected, sizeof raw) == 0);

    vmcu_frame_header_t back;
    CHECK(vmcu_wire_decode_header(raw, &back) == VMCU_WIRE_OK);
    CHECK(back.type == h.type && back.board_id == h.board_id && back.flags == h.flags);
    CHECK(back.seq == h.seq && back.length == h.length && back.timestamp_us == h.timestamp_us);
}

static void test_header_errors(void)
{
    vmcu_frame_header_t h = { .type = 1, .board_id = 1, .length = VMCU_WIRE_MAX_PAYLOAD + 1 };
    uint8_t raw[VMCU_WIRE_HEADER_SIZE];
    vmcu_wire_encode_header(raw, &h);
    vmcu_frame_header_t back;
    CHECK(vmcu_wire_decode_header(raw, &back) == VMCU_WIRE_TOO_LONG);
    raw[0] = 'X';
    CHECK(vmcu_wire_decode_header(raw, &back) == VMCU_WIRE_BAD_MAGIC);
}

static void test_buffers(void)
{
    uint8_t buf[8];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, buf, sizeof buf);
    vmcu_wbuf_u32(&w, 0xDEADBEEFu);
    vmcu_wbuf_u16(&w, 0x1234u);
    CHECK(!w.overflow && w.len == 6);
    vmcu_wbuf_u32(&w, 1u); /* dépasse la capacité */
    CHECK(w.overflow && w.len == 6);

    vmcu_rbuf_t r;
    vmcu_rbuf_init(&r, buf, 6);
    CHECK(vmcu_rbuf_u32(&r) == 0xDEADBEEFu);
    CHECK(vmcu_rbuf_u16(&r) == 0x1234u);
    CHECK(!r.error);
    CHECK(vmcu_rbuf_u8(&r) == 0 && r.error); /* au-delà de la fin */
}

static void test_strings(void)
{
    uint8_t buf[VMCU_WIRE_MAX_STR + 8];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, buf, sizeof buf);
    vmcu_wbuf_str(&w, "carte");
    vmcu_rbuf_t r;
    vmcu_rbuf_init(&r, buf, w.len);
    char out[16];
    vmcu_rbuf_str(&r, out, sizeof out);
    CHECK(strcmp(out, "carte") == 0);

    /* Troncature sans couper un caractère UTF-8 (« é » = 2 octets). */
    char long_text[VMCU_WIRE_MAX_STR + 2];
    memset(long_text, 'a', sizeof long_text);
    long_text[VMCU_WIRE_MAX_STR - 1] = (char)0xC3;
    long_text[VMCU_WIRE_MAX_STR] = (char)0xA9;
    long_text[VMCU_WIRE_MAX_STR + 1] = '\0';
    vmcu_wbuf_init(&w, buf, sizeof buf);
    vmcu_wbuf_str(&w, long_text);
    CHECK(vmcu_wire_get_u16(buf) == VMCU_WIRE_MAX_STR - 1);
}

int main(void)
{
    test_header_bytes();
    test_header_errors();
    test_buffers();
    test_strings();
    printf("test_wire : OK\n");
    return 0;
}
