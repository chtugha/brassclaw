//! Types for Tomedo API requests and responses.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Diagnosis certainty in German medical coding.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosisCertainty {
    /// Gesichert (confirmed)
    G,
    /// Verdacht (suspected)
    V,
    /// Ausgeschlossen (excluded)
    A,
    /// Zustand nach (status post)
    Z,
}

/// All actions the Tomedo tool can perform.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum TomedoAction {
    /// Get the patient ID from the currently active Tomedo window.
    /// Requires the tomedo-monitor daemon to be running.
    GetActivePatientId,

    /// Fetch basic patient information for a given patient ID.
    /// The fields returned are controlled by the `tomedo_patient_fields` secret.
    GetPatientInfo {
        /// Tomedo patient ID (as shown in the practice management system).
        patient_id: String,
    },

    /// Get all ICD-10 diagnoses for a patient in the current quarter.
    GetIcdCodes {
        patient_id: String,
        /// Optional: filter by quarter (1–4). Defaults to current quarter.
        #[serde(default)]
        quarter: Option<u8>,
        /// Optional: filter by year (e.g. 2025). Defaults to current year.
        #[serde(default)]
        year: Option<u32>,
    },

    /// Add an ICD-10 diagnosis to a patient's record.
    AddIcdCode {
        patient_id: String,
        /// ICD-10-GM code (e.g. "E11.9").
        icd_code: String,
        /// Certainty qualifier.
        certainty: DiagnosisCertainty,
        /// Optional free-text note.
        #[serde(default)]
        note: Option<String>,
    },

    /// Delete a specific diagnosis entry.
    DeleteIcdCode {
        patient_id: String,
        /// Internal diagnosis record ID (from GetIcdCodes response).
        diagnosis_id: String,
    },

    /// Find and remove exact ICD-code duplicates (same code + certainty).
    /// Reports what was removed without requiring a separate delete call.
    RemoveDuplicateIcdCodes {
        patient_id: String,
        #[serde(default)]
        quarter: Option<u8>,
        #[serde(default)]
        year: Option<u32>,
    },

    /// Get the billing/invoice items for a patient in a quarter.
    GetInvoices {
        patient_id: String,
        quarter: u8,
        year: u32,
    },

    /// Add a billing number to the patient's invoice.
    AddInvoiceItem {
        patient_id: String,
        /// HZV billing number (e.g. "56544").
        billing_number: String,
        /// Human-readable description of this billing item.
        description: String,
        #[serde(default)]
        quarter: Option<u8>,
        #[serde(default)]
        year: Option<u32>,
    },

    /// Check the invoice completeness against the configured rules.
    /// Returns missing billing numbers with reasons.
    CheckInvoiceCompleteness {
        patient_id: String,
        quarter: u8,
        year: u32,
    },

    /// Check if the patient qualifies for the AOK HZV P4 quality bonus
    /// (billing code 56544, billed twice per quarter when qualified).
    /// Checks:
    ///   1. Whether 3+ P4 ICD codes are already present → full qualification.
    ///   2. Whether 2 P4 codes are present → scan records for a third,
    ///      add it, and flag 56544 to be billed twice.
    CheckP4Qualification {
        patient_id: String,
        #[serde(default)]
        quarter: Option<u8>,
        #[serde(default)]
        year: Option<u32>,
        /// When true, automatically add any found third diagnosis and add 56544.
        #[serde(default)]
        auto_apply: bool,
    },

    /// Scan patient records (Karteikarte / Dokumente) for ICD codes or
    /// clinical keywords that match any HZV-billable code from the HAVG list.
    /// Returns a list of potentially missing billing numbers with match evidence.
    ScanPatientRecordsForBillableItems {
        patient_id: String,
        #[serde(default)]
        quarter: Option<u8>,
        #[serde(default)]
        year: Option<u32>,
    },

    /// List the built-in P4 ICD code list and the HAVG HZV billing codes.
    ListReferenceData {
        /// "p4_icd_codes" | "havg_billing_codes" | "all"
        #[serde(default = "default_reference_data")]
        dataset: String,
    },
}

fn default_reference_data() -> String {
    "all".into()
}

#[derive(Debug, Serialize)]
pub struct ActivePatient {
    pub patient_id: String,
    pub window_title: String,
    pub timestamp_unix: u64,
}

#[derive(Debug, Serialize)]
pub struct PatientInfo {
    pub patient_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_of_birth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insurance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insurance_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insurance_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Diagnosis {
    pub diagnosis_id: String,
    pub icd_code: String,
    pub icd_description: String,
    pub certainty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub quarter: u8,
    pub year: u32,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct IcdCodesResult {
    pub patient_id: String,
    pub quarter: u8,
    pub year: u32,
    pub diagnoses: Vec<Diagnosis>,
    pub total_count: usize,
}

#[derive(Debug, Serialize)]
pub struct DuplicateRemovalResult {
    pub patient_id: String,
    pub quarter: u8,
    pub year: u32,
    pub removed_count: usize,
    pub removed_entries: Vec<Diagnosis>,
    pub remaining_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InvoiceItem {
    pub item_id: String,
    pub billing_number: String,
    pub description: String,
    pub quantity: u32,
    pub date: String,
}

#[derive(Debug, Serialize)]
pub struct InvoiceResult {
    pub patient_id: String,
    pub quarter: u8,
    pub year: u32,
    pub items: Vec<InvoiceItem>,
    pub total_count: usize,
}

#[derive(Debug, Serialize)]
pub struct InvoiceItemAdded {
    pub ok: bool,
    pub item_id: String,
    pub billing_number: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct MissingBillingItem {
    pub billing_number: String,
    pub description: String,
    pub reason: String,
    pub rule_id: String,
}

#[derive(Debug, Serialize)]
pub struct InvoiceCompletenessResult {
    pub patient_id: String,
    pub quarter: u8,
    pub year: u32,
    pub complete: bool,
    pub missing_items: Vec<MissingBillingItem>,
}

#[derive(Debug, Serialize)]
pub struct P4Qualification {
    pub patient_id: String,
    pub quarter: u8,
    pub year: u32,
    pub qualifies: bool,
    pub p4_codes_present: Vec<String>,
    pub p4_codes_count: usize,
    pub threshold: usize,
    pub billing_number: String,
    pub times_to_bill: u8,
    pub status: P4Status,
    pub actions_taken: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum P4Status {
    FullyQualified,
    QualifiedAfterRecordScan,
    TwoDiagnosesMissingThird,
    InsufficientDiagnoses,
    AlreadyBilled,
}

#[derive(Debug, Serialize)]
pub struct BillableItemMatch {
    pub billing_number: String,
    pub description: String,
    pub match_evidence: String,
    pub icd_codes: Vec<String>,
    pub already_billed: bool,
}

#[derive(Debug, Serialize)]
pub struct RecordScanResult {
    pub patient_id: String,
    pub quarter: u8,
    pub year: u32,
    pub matches: Vec<BillableItemMatch>,
    pub recommendation: String,
}

#[derive(Debug, Serialize)]
pub struct ReferenceDataResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p4_icd_codes: Option<Vec<P4IcdEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub havg_billing_codes: Option<Vec<HavgBillingEntry>>,
}

#[derive(Debug, Serialize)]
pub struct P4IcdEntry {
    pub icd_prefix: String,
    pub description: String,
    pub category: String,
}

#[derive(Debug, Serialize)]
pub struct HavgBillingEntry {
    pub billing_number: String,
    pub description: String,
    pub condition_keywords: Vec<String>,
}
