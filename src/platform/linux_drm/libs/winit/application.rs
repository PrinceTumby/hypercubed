use super::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use super::event_loop::ActiveEventLoop;
use super::window::WindowId;

pub trait ApplicationHandler<T: 'static = ()> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop);

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    );

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        _ = (event_loop, cause);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: T) {
        _ = (event_loop, event);
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        _ = (event_loop, device_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        _ = event_loop;
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        _ = event_loop;
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        _ = event_loop;
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        _ = event_loop;
    }
}
