use anyhow::{Context, Result};
use json_patch::{jsonptr::PointerBuf, AddOperation, PatchOperation, ReplaceOperation};

use super::{reference::HttpPartReference, JsonValue, Modifer, Passthrough, Replace};

impl Modifer {
    pub fn to_dst_path(&self, ref_mode: &HttpPartReference) -> Result<PointerBuf> {
        match (self, ref_mode) {
            (_, HttpPartReference::HeaderRegex(_)) => {
                Err(anyhow::anyhow!("header regex cannot write to destination"))
            }
            (_, HttpPartReference::QueryRegex(_)) => {
                Err(anyhow::anyhow!("query regex cannot write to destination"))
            }
            (_, HttpPartReference::Context(_)) => {
                Err(anyhow::anyhow!("context cannot write to destination"))
            }
            (_, HttpPartReference::Header(h)) => Ok(PointerBuf::parse(format!("/header/{}", h)).unwrap()),
            (Modifer::Passthrough(_), HttpPartReference::Query(q)) => {
                Ok(PointerBuf::parse(format!("/query/{}/0", q)).unwrap())
            }
            (Modifer::Replace(_), HttpPartReference::Query(q)) => {
                Ok(PointerBuf::parse(format!("/query/{}", q)).unwrap())
            }
            (_, HttpPartReference::Unsafe(q)) => Ok(q.clone()),
        }
    }

    pub fn apply(&self, src: &JsonValue, dst: &mut JsonValue) -> Result<()> {
        match self {
            Modifer::Passthrough(Passthrough::Implicit { from }) => {
                let founds = from.resolve(src);
                for v in founds {
                    let dst_path = self.to_dst_path(from)?;
                    json_patch::patch(
                        dst,
                        &[PatchOperation::Add(AddOperation {
                            path: dst_path.clone(),
                            value: v.clone(),
                        })],
                    )
                    .context("passthrough patch failed")?;
                }
                Ok(())
            }
            Modifer::Passthrough(Passthrough::Explicit { from, to }) => {
                let dst_path = self.to_dst_path(to)?;
                let founds = from.resolve(src);
                for v in founds {
                    json_patch::patch(
                        dst,
                        &[PatchOperation::Add(AddOperation {
                            path: dst_path.clone(),
                            value: v.clone(),
                        })],
                    )
                    .context("passthrough patch failed")?;
                }
                Ok(())
            }
            Modifer::Replace(Replace { to, value }) => {
                let dst_path = self.to_dst_path(to)?;
                json_patch::patch(
                    dst,
                    &[PatchOperation::Replace(ReplaceOperation {
                        path: dst_path.clone(),
                        value: value.clone(),
                    })],
                )
                .context("replace patch failed")?;
                Ok(())
            }
        }
    }
}
