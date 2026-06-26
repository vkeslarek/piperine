use crate::digital::logic::LogicValue;
use crate::digital::net::{DigitalNet, DigitalEvent};
use crate::digital::ffi::*;
use crate::digital::state::DigitalDevice;
use std::collections::{HashSet, BinaryHeap};
use std::cmp::Reverse;
use std::os::raw::{c_void, c_double};

pub struct FfiDigitalDevice {
    pub instance_id: usize,
    pub descriptor: *const DigitalDescriptor,
    pub inst_data: *mut c_void,
    pub model_data: *mut c_void,
    pub input_net_ids: Vec<DigitalNet>,
    pub output_net_ids: Vec<DigitalNet>,
    inputs_buffer: Vec<u8>,
    outputs_buffer: Vec<u8>,
}

unsafe impl Send for FfiDigitalDevice {}
unsafe impl Sync for FfiDigitalDevice {}

struct EventSinkContext<'a> {
    queue: &'a mut BinaryHeap<Reverse<DigitalEvent>>,
    current_time: f64,
    output_net_ids: &'a [DigitalNet],
    instance_id: usize,
}

unsafe extern "C" fn schedule_cb(handle: *mut c_void, port_idx: u32, value: u8, delay: c_double) {
    let ctx = unsafe { &mut *(handle as *mut EventSinkContext) };
    let net = ctx.output_net_ids[port_idx as usize];
    let logic_val = match value {
        DIGITAL_LOGIC_ZERO => LogicValue::Zero,
        DIGITAL_LOGIC_ONE => LogicValue::One,
        DIGITAL_LOGIC_X => LogicValue::X,
        _ => LogicValue::Z,
    };
    ctx.queue.push(Reverse(DigitalEvent {
        time: ctx.current_time + delay,
        net,
        value: logic_val,
        source: ctx.instance_id,
        seq: 0,
    }));
}

unsafe extern "C" fn cancel_cb(handle: *mut c_void, port_idx: u32) {
    let ctx = unsafe { &mut *(handle as *mut EventSinkContext) };
    let net = ctx.output_net_ids[port_idx as usize];

    let mut new_queue = BinaryHeap::with_capacity(ctx.queue.len());
    while let Some(Reverse(e)) = ctx.queue.pop() {
        if !(e.net == net && e.source == ctx.instance_id) {
            new_queue.push(Reverse(e));
        }
    }
    *ctx.queue = new_queue;
}

impl FfiDigitalDevice {
    pub fn new(
        instance_id: usize,
        descriptor: *const DigitalDescriptor,
        inst_data: *mut c_void,
        model_data: *mut c_void,
        input_net_ids: Vec<DigitalNet>,
        output_net_ids: Vec<DigitalNet>,
    ) -> Self {
        let num_inputs = input_net_ids.len();
        let num_outputs = output_net_ids.len();
        Self {
            instance_id,
            descriptor,
            inst_data,
            model_data,
            input_net_ids,
            output_net_ids,
            inputs_buffer: vec![DIGITAL_LOGIC_X; num_inputs],
            outputs_buffer: vec![DIGITAL_LOGIC_X; num_outputs],
        }
    }
}

impl DigitalDevice for FfiDigitalDevice {
    fn has_input_on(&self, changed_nets: &HashSet<DigitalNet>) -> bool {
        self.input_net_ids.iter().any(|net| changed_nets.contains(net))
    }

    fn input_nets(&self) -> &[DigitalNet] { &self.input_net_ids }
    fn output_nets(&self) -> &[DigitalNet] { &self.output_net_ids }

    fn eval(&mut self, current_time: f64, nets: &[LogicValue], event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>) {
        let desc = unsafe { &*self.descriptor };
        if let Some(eval_fn) = desc.eval {
            for (i, net) in self.input_net_ids.iter().enumerate() {
                self.inputs_buffer[i] = nets[net.0] as u8;
            }

            let mut ctx = EventSinkContext {
                queue: event_queue,
                current_time,
                output_net_ids: &self.output_net_ids,
                instance_id: self.instance_id,
            };

            let mut sink = DigitalEventSink {
                handle: &mut ctx as *mut _ as *mut c_void,
                schedule: Some(schedule_cb),
                cancel: Some(cancel_cb),
            };

            unsafe {
                eval_fn(
                    self.inst_data,
                    self.model_data,
                    self.inputs_buffer.as_ptr(),
                    self.outputs_buffer.as_mut_ptr(),
                    &mut sink,
                    current_time,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    unsafe extern "C" fn mock_inverter_eval(
        _inst_data: *mut c_void,
        _model_data: *mut c_void,
        inputs: *const u8,
        _outputs: *mut u8,
        event_sink: *mut DigitalEventSink,
        _current_time: c_double,
    ) -> u32 {
        let in_val = unsafe { *inputs };
        let out_val = match in_val {
            DIGITAL_LOGIC_ZERO => DIGITAL_LOGIC_ONE,
            DIGITAL_LOGIC_ONE => DIGITAL_LOGIC_ZERO,
            _ => DIGITAL_LOGIC_X,
        };

        let sink = unsafe { &mut *event_sink };
        if let Some(sched) = sink.schedule {
            unsafe { sched(sink.handle, 0, out_val, 5.0) };
        }

        0
    }

    #[test]
    fn test_ffi_digital_device_eval() {
        let desc = DigitalDescriptor {
            name: b"mock_inv\0".as_ptr() as *const _,
            num_ports: 2,
            num_params: 0,
            ports: ptr::null(),
            params: ptr::null(),
            instance_size: 0,
            model_size: 0,
            setup_model: None,
            setup_instance: None,
            eval: Some(mock_inverter_eval),
            access: None,
        };

        let mut device = FfiDigitalDevice::new(
            42,
            &desc,
            ptr::null_mut(),
            ptr::null_mut(),
            vec![DigitalNet(0)],
            vec![DigitalNet(1)],
        );

        let nets = vec![LogicValue::One, LogicValue::X];
        let mut queue = BinaryHeap::new();

        device.eval(10.0, &nets, &mut queue);

        assert_eq!(queue.len(), 1);
        let ev = queue.pop().unwrap().0;
        assert_eq!(ev.time, 15.0);
        assert_eq!(ev.value, LogicValue::Zero);
        assert_eq!(ev.net, DigitalNet(1));
        assert_eq!(ev.source, 42);
    }
}
