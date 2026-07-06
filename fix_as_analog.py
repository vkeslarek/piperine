import os

path = "crates/piperine-codegen/src/device/mod.rs"
with open(path, "r") as f:
    text = f.read()

target = """impl Device for PiperineDevice {
    fn device_name(&self) -> &str {
        &self.label
    }
}"""

replacement = """impl Device for PiperineDevice {
    fn device_name(&self) -> &str {
        &self.label
    }
    fn as_analog(&mut self) -> Option<&mut dyn AnalogDevice> { Some(self) }
    fn as_analog_ref(&self) -> Option<&dyn AnalogDevice> { Some(self) }
    fn as_digital(&mut self) -> Option<&mut dyn DigitalDevice> { Some(self) }
    fn as_digital_ref(&self) -> Option<&dyn DigitalDevice> { Some(self) }
}"""

if target in text:
    text = text.replace(target, replacement)
    with open(path, "w") as f:
        f.write(text)
    print("SUCCESS")
else:
    print("TARGET NOT FOUND")

