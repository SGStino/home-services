use std::borrow::Cow;

pub const VENDOR_SPECIFIC_ATTR_START: u64 = 0xFFF8;

pub fn is_vendor_specific_attribute(attribute: u64) -> bool {
    attribute >= VENDOR_SPECIFIC_ATTR_START
}

pub fn cluster_name(cluster: u64) -> &'static str {
    match cluster {
        3 => "Identify",
        4 => "Groups",
        5 => "Scenes",
        6 => "OnOff",
        8 => "LevelControl",
        29 => "Descriptor",
        30 => "Binding",
        31 => "AccessControl",
        40 => "BasicInformation",
        43 => "LocalizationConfiguration",
        44 => "TimeFormatLocalization",
        45 => "UnitLocalization",
        47 => "PowerSource",
        48 => "GeneralCommissioning",
        49 => "NetworkCommissioning",
        50 => "DiagnosticLogs",
        51 => "GeneralDiagnostics",
        52 => "SoftwareDiagnostics",
        53 => "ThreadNetworkDiagnostics",
        54 => "WiFiNetworkDiagnostics",
        55 => "EthernetNetworkDiagnostics",
        60 => "AdministratorCommissioning",
        62 => "OperationalCredentials",
        63 => "GroupKeyManagement",
        64 => "FixedLabel",
        65 => "UserLabel",
        144 => "ElectricalEnergyMeasurement",
        145 => "ElectricalPowerMeasurement",
        768 => "ColorControl",
        1026 => "TemperatureMeasurement",
        1029 => "RelativeHumidityMeasurement",
        _ => "Cluster",
    }
}

pub fn attribute_name(cluster: u64, attribute: u64) -> Option<&'static str> {
    match (cluster, attribute) {
        (6, 0) => Some("OnOff"),
        (6, 1) => Some("GlobalSceneControl"),
        (6, 2) => Some("OnTime"),
        (6, 3) => Some("OffWaitTime"),
        (6, 4) => Some("StartUpOnOff"),
        (8, 0) => Some("CurrentLevel"),
        (8, 1) => Some("RemainingTime"),
        (8, 2) => Some("MinLevel"),
        (8, 3) => Some("MaxLevel"),
        (8, 15) => Some("Options"),
        (8, 17) => Some("OnOffTransitionTime"),
        (8, 18) => Some("OnLevel"),
        (8, 19) => Some("OnTransitionTime"),
        (8, 20) => Some("OffTransitionTime"),
        (8, 21) => Some("DefaultMoveRate"),
        (29, 0) => Some("DeviceTypeList"),
        (29, 1) => Some("ServerList"),
        (29, 2) => Some("ClientList"),
        (29, 3) => Some("PartsList"),
        (40, 0) => Some("DataModelRevision"),
        (40, 1) => Some("VendorName"),
        (40, 2) => Some("VendorID"),
        (40, 3) => Some("ProductName"),
        (40, 4) => Some("ProductID"),
        (40, 5) => Some("NodeLabel"),
        (40, 6) => Some("Location"),
        (40, 7) => Some("HardwareVersion"),
        (40, 8) => Some("HardwareVersionString"),
        (40, 9) => Some("SoftwareVersion"),
        (40, 10) => Some("SoftwareVersionString"),
        (40, 11) => Some("ManufacturingDate"),
        (40, 12) => Some("PartNumber"),
        (40, 13) => Some("ProductURL"),
        (40, 14) => Some("ProductLabel"),
        (40, 15) => Some("SerialNumber"),
        (40, 16) => Some("LocalConfigDisabled"),
        (40, 17) => Some("Reachable"),
        (40, 18) => Some("UniqueID"),
        (40, 19) => Some("CapabilityMinima"),
        (43, 0) => Some("ActiveLocale"),
        (43, 1) => Some("SupportedLocales"),
        (44, 0) => Some("HourFormat"),
        (44, 1) => Some("ActiveCalendarType"),
        (44, 2) => Some("SupportedCalendarTypes"),
        (45, 0) => Some("TemperatureUnit"),
        (47, 0) => Some("Status"),
        (47, 1) => Some("Order"),
        (47, 2) => Some("Description"),
        (47, 3) => Some("WiredAssessedInputVoltage"),
        (47, 4) => Some("WiredAssessedInputFrequency"),
        (47, 5) => Some("WiredCurrentType"),
        (47, 11) => Some("BatteryVoltage"),
        (47, 12) => Some("BatteryPercent"),
        (47, 14) => Some("BatteryChargeLevel"),
        (48, 0) => Some("Breadcrumb"),
        (48, 1) => Some("BasicCommissioningInfo"),
        (48, 2) => Some("RegulatoryConfig"),
        (48, 3) => Some("LocationCapability"),
        (48, 4) => Some("SupportsConcurrentConnection"),
        (49, 0) => Some("MaxNetworks"),
        (49, 1) => Some("Networks"),
        (49, 2) => Some("ScanMaxTimeSeconds"),
        (49, 3) => Some("ConnectMaxTimeSeconds"),
        (49, 4) => Some("InterfaceEnabled"),
        (49, 5) => Some("LastNetworkingStatus"),
        (49, 6) => Some("LastNetworkID"),
        (49, 7) => Some("LastConnectErrorValue"),
        (768, 0) => Some("CurrentHue"),
        (768, 1) => Some("CurrentSaturation"),
        (768, 3) => Some("CurrentX"),
        (768, 4) => Some("CurrentY"),
        (768, 7) => Some("ColorTemperatureMireds"),
        (768, 8) => Some("ColorMode"),
        (1026, 0) => Some("MeasuredValue"),
        (1026, 1) => Some("MinMeasuredValue"),
        (1026, 2) => Some("MaxMeasuredValue"),
        (1029, 0) => Some("MeasuredValue"),
        (1029, 1) => Some("MinMeasuredValue"),
        (1029, 2) => Some("MaxMeasuredValue"),
        (144, 0) => Some("Accuracy"),
        (144, 1) => Some("CumulativeEnergyImported"),
        (144, 2) => Some("CumulativeEnergyExported"),
        (144, 3) => Some("PeriodicEnergyImported"),
        (144, 4) => Some("PeriodicEnergyExported"),
        (145, 0) => Some("PowerMode"),
        (145, 1) => Some("NumberOfMeasurementTypes"),
        (145, 4) => Some("Voltage"),
        (145, 5) => Some("ActiveCurrent"),
        (145, 6) => Some("ReactiveCurrent"),
        (145, 7) => Some("ApparentCurrent"),
        (145, 8) => Some("ActivePower"),
        (145, 9) => Some("ReactivePower"),
        (145, 10) => Some("ApparentPower"),
        (145, 11) => Some("RMSVoltage"),
        (145, 12) => Some("RMSCurrent"),
        (145, 13) => Some("RMSPower"),
        (145, 14) => Some("Frequency"),
        (145, 17) => Some("PowerFactor"),
        _ => None,
    }
}

pub fn friendly_name(endpoint: u64, cluster: u64, attribute: u64) -> String {
    let base = match attribute_name(cluster, attribute) {
        Some(name) => split_camel_case(name),
        None => {
            if cluster_name(cluster) == "Cluster" {
                format!("Cluster {} attribute {}", cluster, attribute)
            } else {
                format!("{} attribute {}", split_camel_case(cluster_name(cluster)), attribute)
            }
        }
    };

    if endpoint == 0 {
        base
    } else {
        format!("{} (endpoint {})", base, endpoint)
    }
}

pub fn capability_id(endpoint: u64, cluster: u64, attribute: u64) -> String {
    let cluster_slug = if cluster_name(cluster) == "Cluster" {
        Cow::Owned(format!("cluster_{}", cluster))
    } else {
        slugify(cluster_name(cluster))
    };
    let attr_slug = match attribute_name(cluster, attribute) {
        Some(name) => slugify(name),
        None => Cow::Owned(format!("attr_{}", attribute)),
    };

    format!("matter_ep{}_{}_{}", endpoint, cluster_slug, attr_slug)
}

pub fn unit_for_attribute(cluster: u64, attribute: u64) -> Option<&'static str> {
    match (cluster, attribute) {
        (47, 11) => Some("V"),
        (47, 12) => Some("%"),
        (1026, 0 | 1 | 2) => Some("°C"),
        (1029, 0 | 1 | 2) => Some("%"),
        (144, 1 | 2 | 3 | 4) => Some("kWh"),
        (145, 4 | 11) => Some("V"),
        (145, 5 | 6 | 7 | 12) => Some("A"),
        (145, 8 | 9 | 10 | 13) => Some("W"),
        (145, 14) => Some("Hz"),
        (145, 17) => Some("%"),
        _ => None,
    }
}

fn split_camel_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    let mut prev_is_lower_or_digit = false;

    for c in input.chars() {
        if c.is_uppercase() && prev_is_lower_or_digit {
            out.push(' ');
        }
        out.push(c);
        prev_is_lower_or_digit = c.is_ascii_lowercase() || c.is_ascii_digit();
    }

    out
}

fn slugify(input: &str) -> Cow<'_, str> {
    let mut out = String::with_capacity(input.len() + 8);
    let mut prev_is_lower_or_digit = false;

    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && prev_is_lower_or_digit {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            prev_is_lower_or_digit = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else {
            if !out.ends_with('_') {
                out.push('_');
            }
            prev_is_lower_or_digit = false;
        }
    }

    while out.ends_with('_') {
        out.pop();
    }

    if out.is_empty() {
        Cow::Borrowed("value")
    } else {
        Cow::Owned(out)
    }
}
