#include <stdint.h>

// Window Event

typedef enum {
    ResizedWindowEvent,
    CloseRequestedWindowEvent,
} ClientWindowEventType;

typedef struct {
    uint32_t width;
    uint32_t height;
} ClientResizedWindowEventData;

typedef union {
    ClientResizedWindowEventData resized_data;
    // void close_requested_data;
} ClientWindowEventData;

typedef struct {
    uint8_t event_type;
    ClientWindowEventData event_data;
} ClientWindowEvent;

// Device Event

typedef enum {
    KeyDeviceEvent,
} ClientDeviceEventType;

typedef enum {
    PressedElementState = 0,
    ReleasedElementState = 1,
} ClientElementState;

typedef struct {
    uint16_t mac_scancode;
    uint8_t element_state;
} ClientDeviceKeyEventData;

typedef union {
    ClientDeviceKeyEventData key_data;
} ClientDeviceEventData;

typedef struct {
    uint8_t event_type;
    ClientDeviceEventData event_data;
} ClientDeviceEvent;

// Client Event

typedef enum {
    PollNewEvents,
    WindowEvent,
    DeviceEvent,
} ClientEventType;

typedef union {
    // void poll_new_events_data;
    ClientWindowEvent window_event_data;
    ClientDeviceEvent device_event_data;
} ClientEventData;

typedef struct {
    uint8_t event_type;
    ClientEventData event_data;
} ClientEvent;
