/*
 * libvmcu/platform/vmcu_platform_win32.c — Couche système Windows (MinGW-w64).
 *
 * Canal : named pipe \\.\pipe\..., ouvert en E/S asynchrones (overlapped).
 * Sur un handle synchrone, Windows sérialise les opérations : une lecture
 * bloquée dans le fil E/S empêcherait toute écriture des autres fils. Chaque
 * sens a donc son OVERLAPPED et son événement.
 *
 * Sommeil : timer haute résolution (CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
 * Windows 10 1803+). Sans lui, Sleep(1) dure environ 15 ms (section 5.5).
 */
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00
#endif
#define WIN32_LEAN_AND_MEAN

#include "vmcu_platform.h"

#include <windows.h>
#include <process.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef CREATE_WAITABLE_TIMER_HIGH_RESOLUTION
#define CREATE_WAITABLE_TIMER_HIGH_RESOLUTION 0x00000002
#endif

#define PIPE_BUFFER_SIZE 65536u
#define PIPE_NAME_MAX    256u

/* --- Canal ------------------------------------------------------------------- */

struct vmcu_plat_conn {
    HANDLE pipe;
    HANDLE read_event;
    HANDLE write_event;
    HANDLE stop_event; /* signalé par vmcu_plat_conn_shutdown */
};

struct vmcu_plat_listener {
    char   name[PIPE_NAME_MAX];
    HANDLE pending; /* instance de pipe créée, en attente de client */
};

static vmcu_plat_conn_t *conn_new(HANDLE pipe)
{
    vmcu_plat_conn_t *c = calloc(1, sizeof *c);
    if (c == NULL) {
        CloseHandle(pipe);
        return NULL;
    }
    c->pipe = pipe;
    c->read_event = CreateEventW(NULL, TRUE, FALSE, NULL);
    c->write_event = CreateEventW(NULL, TRUE, FALSE, NULL);
    c->stop_event = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (c->read_event == NULL || c->write_event == NULL || c->stop_event == NULL) {
        vmcu_plat_conn_close(c);
        return NULL;
    }
    return c;
}

vmcu_plat_result_t vmcu_plat_connect(const char *address, vmcu_plat_conn_t **out)
{
    HANDLE h = CreateFileA(address, GENERIC_READ | GENERIC_WRITE, 0, NULL, OPEN_EXISTING,
                           FILE_FLAG_OVERLAPPED, NULL);
    if (h == INVALID_HANDLE_VALUE) {
        DWORD err = GetLastError();
        if (err == ERROR_PIPE_BUSY) {
            (void)WaitNamedPipeA(address, 100);
            return VMCU_PLAT_RETRY;
        }
        return (err == ERROR_FILE_NOT_FOUND) ? VMCU_PLAT_RETRY : VMCU_PLAT_ERROR;
    }
    *out = conn_new(h);
    return (*out != NULL) ? VMCU_PLAT_OK : VMCU_PLAT_ERROR;
}

/* Attend la fin d'une opération ; abandonne si stop_event est signalé. */
static BOOL wait_io(vmcu_plat_conn_t *c, OVERLAPPED *ov, DWORD *done)
{
    HANDLE events[2] = { ov->hEvent, c->stop_event };
    DWORD r = WaitForMultipleObjects(2, events, FALSE, INFINITE);
    if (r != WAIT_OBJECT_0) {
        (void)CancelIoEx(c->pipe, ov);
        (void)GetOverlappedResult(c->pipe, ov, done, TRUE);
        return FALSE;
    }
    return GetOverlappedResult(c->pipe, ov, done, FALSE);
}

int vmcu_plat_write_all(vmcu_plat_conn_t *conn, const void *data, size_t len)
{
    const uint8_t *p = data;
    while (len > 0) {
        DWORD chunk = (len > 0x10000000u) ? 0x10000000u : (DWORD)len;
        OVERLAPPED ov;
        memset(&ov, 0, sizeof ov);
        ov.hEvent = conn->write_event;
        DWORD done = 0;
        if (!WriteFile(conn->pipe, p, chunk, NULL, &ov)) {
            if (GetLastError() != ERROR_IO_PENDING) {
                return -1;
            }
        }
        if (!wait_io(conn, &ov, &done) || done == 0) {
            return -1;
        }
        p += done;
        len -= done;
    }
    return 0;
}

long vmcu_plat_read(vmcu_plat_conn_t *conn, void *buf, size_t len)
{
    if (WaitForSingleObject(conn->stop_event, 0) == WAIT_OBJECT_0) {
        return -1;
    }
    DWORD want = (len > 0x10000000u) ? 0x10000000u : (DWORD)len;
    OVERLAPPED ov;
    memset(&ov, 0, sizeof ov);
    ov.hEvent = conn->read_event;
    DWORD done = 0;
    if (!ReadFile(conn->pipe, buf, want, NULL, &ov)) {
        DWORD err = GetLastError();
        if (err == ERROR_BROKEN_PIPE || err == ERROR_PIPE_NOT_CONNECTED) {
            return 0;
        }
        if (err != ERROR_IO_PENDING) {
            return -1;
        }
    }
    if (!wait_io(conn, &ov, &done)) {
        DWORD err = GetLastError();
        return (err == ERROR_BROKEN_PIPE || err == ERROR_PIPE_NOT_CONNECTED) ? 0 : -1;
    }
    return (long)done;
}

void vmcu_plat_conn_shutdown(vmcu_plat_conn_t *conn)
{
    (void)SetEvent(conn->stop_event);
}

void vmcu_plat_conn_close(vmcu_plat_conn_t *conn)
{
    if (conn == NULL) {
        return;
    }
    if (conn->pipe != NULL && conn->pipe != INVALID_HANDLE_VALUE) {
        CloseHandle(conn->pipe);
    }
    if (conn->read_event != NULL) {
        CloseHandle(conn->read_event);
    }
    if (conn->write_event != NULL) {
        CloseHandle(conn->write_event);
    }
    if (conn->stop_event != NULL) {
        CloseHandle(conn->stop_event);
    }
    free(conn);
}

static HANDLE create_pipe_instance(const char *name)
{
    return CreateNamedPipeA(name, PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT, 1,
                            PIPE_BUFFER_SIZE, PIPE_BUFFER_SIZE, 0, NULL);
}

vmcu_plat_result_t vmcu_plat_listen(const char *address, vmcu_plat_listener_t **out)
{
    vmcu_plat_listener_t *l = calloc(1, sizeof *l);
    if (l == NULL || strlen(address) >= sizeof l->name) {
        free(l);
        return VMCU_PLAT_ERROR;
    }
    strcpy(l->name, address);
    l->pending = create_pipe_instance(l->name);
    if (l->pending == INVALID_HANDLE_VALUE) {
        free(l);
        return VMCU_PLAT_ERROR;
    }
    *out = l;
    return VMCU_PLAT_OK;
}

vmcu_plat_result_t vmcu_plat_accept(vmcu_plat_listener_t *listener, vmcu_plat_conn_t **out)
{
    HANDLE h = listener->pending;
    OVERLAPPED ov;
    memset(&ov, 0, sizeof ov);
    ov.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (ov.hEvent == NULL) {
        return VMCU_PLAT_ERROR;
    }
    BOOL connected = ConnectNamedPipe(h, &ov);
    if (!connected) {
        DWORD err = GetLastError();
        if (err == ERROR_IO_PENDING) {
            DWORD dummy = 0;
            connected = GetOverlappedResult(h, &ov, &dummy, TRUE);
        } else {
            connected = (err == ERROR_PIPE_CONNECTED);
        }
    }
    CloseHandle(ov.hEvent);
    if (!connected) {
        return VMCU_PLAT_ERROR;
    }
    listener->pending = create_pipe_instance(listener->name);
    *out = conn_new(h);
    return (*out != NULL) ? VMCU_PLAT_OK : VMCU_PLAT_ERROR;
}

void vmcu_plat_listener_close(vmcu_plat_listener_t *listener)
{
    if (listener != NULL) {
        if (listener->pending != INVALID_HANDLE_VALUE) {
            CloseHandle(listener->pending);
        }
        free(listener);
    }
}

void vmcu_plat_test_address(char *out, size_t cap)
{
    static unsigned counter = 0;
    (void)snprintf(out, cap, "\\\\.\\pipe\\vmcu-test-%lu-%u",
                   (unsigned long)GetCurrentProcessId(), counter++);
}

/* --- Temps ---------------------------------------------------------------------- */

uint64_t vmcu_plat_monotonic_ns(void)
{
    static LARGE_INTEGER freq;
    if (freq.QuadPart == 0) {
        (void)QueryPerformanceFrequency(&freq);
    }
    LARGE_INTEGER now;
    (void)QueryPerformanceCounter(&now);
    uint64_t c = (uint64_t)now.QuadPart;
    uint64_t f = (uint64_t)freq.QuadPart;
    return (c / f) * 1000000000ull + (c % f) * 1000000000ull / f;
}

void vmcu_plat_sleep_ns(uint64_t ns)
{
    static _Thread_local HANDLE timer = NULL;
    if (timer == NULL) {
        timer = CreateWaitableTimerExW(NULL, NULL, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                                       TIMER_ALL_ACCESS);
        if (timer == NULL) { /* Windows antérieur à 10 1803 */
            timer = CreateWaitableTimerExW(NULL, NULL, 0, TIMER_ALL_ACCESS);
        }
    }
    if (timer == NULL) {
        Sleep((DWORD)((ns + 999999u) / 1000000u));
        return;
    }
    LARGE_INTEGER due;
    uint64_t ticks = ns / 100u; /* unités de 100 ns, délai relatif = valeur négative */
    due.QuadPart = -(LONGLONG)((ticks > 0) ? ticks : 1);
    if (SetWaitableTimer(timer, &due, 0, NULL, NULL, FALSE)) {
        (void)WaitForSingleObject(timer, INFINITE);
    }
}

/* --- Fils ------------------------------------------------------------------------- */

struct vmcu_plat_thread {
    HANDLE              handle;
    DWORD               id;
    vmcu_plat_thread_fn fn;
    void               *arg;
};

static unsigned __stdcall thread_trampoline(void *p)
{
    vmcu_plat_thread_t *t = p;
    t->fn(t->arg);
    return 0;
}

vmcu_plat_thread_t *vmcu_plat_thread_start(vmcu_plat_thread_fn fn, void *arg)
{
    vmcu_plat_thread_t *t = calloc(1, sizeof *t);
    if (t == NULL) {
        return NULL;
    }
    t->fn = fn;
    t->arg = arg;
    unsigned id = 0;
    uintptr_t h = _beginthreadex(NULL, 0, thread_trampoline, t, 0, &id);
    if (h == 0) {
        free(t);
        return NULL;
    }
    t->handle = (HANDLE)h;
    t->id = id;
    return t;
}

void vmcu_plat_thread_join(vmcu_plat_thread_t *thread)
{
    if (thread != NULL) {
        (void)WaitForSingleObject(thread->handle, INFINITE);
        CloseHandle(thread->handle);
        free(thread);
    }
}

bool vmcu_plat_thread_is_current(const vmcu_plat_thread_t *thread)
{
    return thread != NULL && thread->id == GetCurrentThreadId();
}

/* CRITICAL_SECTION est récursive par nature : `recursive` n'a pas d'effet. */
struct vmcu_plat_mutex {
    CRITICAL_SECTION cs;
};

vmcu_plat_mutex_t *vmcu_plat_mutex_create(bool recursive)
{
    (void)recursive;
    vmcu_plat_mutex_t *mx = malloc(sizeof *mx);
    if (mx != NULL) {
        InitializeCriticalSection(&mx->cs);
    }
    return mx;
}

void vmcu_plat_mutex_lock(vmcu_plat_mutex_t *mutex)
{
    EnterCriticalSection(&mutex->cs);
}

void vmcu_plat_mutex_unlock(vmcu_plat_mutex_t *mutex)
{
    LeaveCriticalSection(&mutex->cs);
}

struct vmcu_plat_cond {
    CONDITION_VARIABLE cv;
};

vmcu_plat_cond_t *vmcu_plat_cond_create(void)
{
    vmcu_plat_cond_t *c = malloc(sizeof *c);
    if (c != NULL) {
        InitializeConditionVariable(&c->cv);
    }
    return c;
}

void vmcu_plat_cond_wait(vmcu_plat_cond_t *cond, vmcu_plat_mutex_t *mutex)
{
    (void)SleepConditionVariableCS(&cond->cv, &mutex->cs, INFINITE);
}

bool vmcu_plat_cond_timedwait(vmcu_plat_cond_t *cond, vmcu_plat_mutex_t *mutex, uint64_t timeout_ns)
{
    uint64_t ms = (timeout_ns + 999999u) / 1000000u;
    if (ms >= INFINITE) {
        ms = INFINITE - 1u;
    }
    if (SleepConditionVariableCS(&cond->cv, &mutex->cs, (DWORD)ms)) {
        return true;
    }
    return GetLastError() != ERROR_TIMEOUT;
}

void vmcu_plat_cond_signal(vmcu_plat_cond_t *cond)
{
    WakeConditionVariable(&cond->cv);
}

void vmcu_plat_cond_broadcast(vmcu_plat_cond_t *cond)
{
    WakeAllConditionVariable(&cond->cv);
}

/* --- Processus ------------------------------------------------------------------------ */

uint32_t vmcu_plat_pid(void)
{
    return (uint32_t)GetCurrentProcessId();
}

int vmcu_plat_setenv(const char *name, const char *value)
{
    return _putenv_s(name, value) == 0 ? 0 : -1;
}

/* Lecture de l'état EcoQoS : GetProcessInformation est chargée dynamiquement
 * (Windows 8+), sans dépendre de la version des en-têtes MinGW. */
typedef struct {
    ULONG Version;
    ULONG ControlMask;
    ULONG StateMask;
} vmcu_power_throttling_t;

typedef BOOL (WINAPI *vmcu_get_process_information_fn)(HANDLE, int, LPVOID, DWORD);

void vmcu_plat_priority(char *out, size_t cap)
{
    const char *klass;
    switch (GetPriorityClass(GetCurrentProcess())) {
    case IDLE_PRIORITY_CLASS:         klass = "basse (idle)"; break;
    case BELOW_NORMAL_PRIORITY_CLASS: klass = "inférieure à la normale"; break;
    case NORMAL_PRIORITY_CLASS:       klass = "normale"; break;
    case ABOVE_NORMAL_PRIORITY_CLASS: klass = "supérieure à la normale"; break;
    case HIGH_PRIORITY_CLASS:         klass = "haute"; break;
    case REALTIME_PRIORITY_CLASS:     klass = "temps réel"; break;
    default:                          klass = "inconnue"; break;
    }
    const char *eco = "non disponible";
    HMODULE kernel = GetModuleHandleW(L"kernel32.dll");
    FARPROC proc = (kernel != NULL) ? GetProcAddress(kernel, "GetProcessInformation") : NULL;
    if (proc != NULL) {
        vmcu_get_process_information_fn get_info;
        memcpy(&get_info, &proc, sizeof get_info);
        vmcu_power_throttling_t state = { 1, 0, 0 }; /* PROCESS_POWER_THROTTLING_CURRENT_VERSION */
        if (get_info(GetCurrentProcess(), 4 /* ProcessPowerThrottling */, &state, sizeof state)) {
            eco = ((state.ControlMask & 1u) != 0 && (state.StateMask & 1u) != 0) ? "actif" : "inactif";
        }
    }
    (void)snprintf(out, cap, "classe de priorité %s, EcoQoS %s", klass, eco);
}
