use piperine_circuit::{
    ConnectionMap, ElaborationError, HardwareDefinition, HardwareInstance,
    ParameterDefinition, ParameterMap, PortDefinition,
};

#[derive(Debug)]
pub struct OsdiHardwareDefinition {
    /// VA module name — becomes the ngspice model name after `pre_osdi`.
    pub module_name: String,
    /// Port names in declaration order (matches the VA `inout` list).
    pub port_names: Vec<String>,
    /// Parameter definitions with defaults extracted from the parsed VA source.
    pub parameter_definitions: Vec<ParameterDefinition>,
}

impl HardwareDefinition for OsdiHardwareDefinition {
    fn name(&self) -> &str { &self.module_name }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &self.parameter_definitions }

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let nets: Vec<String> = self.port_names
            .iter()
            .map(|port| {
                connections.get(port).cloned().ok_or_else(|| {
                    ElaborationError::ConnectionError {
                        instance: instance_name.to_string(),
                        detail: format!("port `{port}` not connected"),
                    }
                })
            })
            .collect::<Result<_, _>>()?;

        Ok(Box::new(OsdiInstance {
            instance_name: instance_name.to_string(),
            model_name: self.module_name.clone(),
            nets,
            parameters: parameters.clone(),
        }))
    }
}

#[derive(Debug)]
struct OsdiInstance {
    instance_name: String,
    model_name: String,
    nets: Vec<String>,
    parameters: ParameterMap,
}

impl HardwareInstance for OsdiInstance {
    fn instance_name(&self) -> &str { &self.instance_name }

    fn spice_lines(&self) -> Vec<String> {
        // .model <modelname> <osdi_type> [param=val ...]
        // ngspice requires a .model card that maps the model name to the OSDI device type.
        // The OSDI device type is the VA module name as registered by the "osdi" command.
        // Parameters go on the .model line (they are model parameters, not instance params).
        let mut model_parts = vec![
            ".model".to_string(),
            self.instance_name.clone(), // unique model name per instance
            self.model_name.clone(),    // OSDI device type = VA module name
        ];
        for (key, val) in &self.parameters {
            model_parts.push(format!("{key}={val}"));
        }

        // N<name> <node1> ... <nodeN> <modelname>
        let mut inst_parts = vec![format!("N{}", self.instance_name)];
        inst_parts.extend(self.nets.clone());
        inst_parts.push(self.instance_name.clone()); // reference own .model card

        vec![model_parts.join(" "), inst_parts.join(" ")]
    }
}
