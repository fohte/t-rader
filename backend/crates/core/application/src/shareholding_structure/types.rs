use core_domain::holdings::{
    CrossShareholdingDocument, LargeVolumeShareholdingDocument, MajorShareholderDocument,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ShareholdingStructureBySymbol {
    pub large_volume_reports: Vec<LargeVolumeShareholdingDocument>,
    pub major_shareholders: Option<MajorShareholderDocument>,
    pub cross_shareholdings: Option<CrossShareholdingDocument>,
}
