/*
 *
 *    Copyright (c) 2020-2022 Project CHIP Authors
 *
 *    Licensed under the Apache License, Version 2.0 (the "License");
 *    you may not use this file except in compliance with the License.
 *    You may obtain a copy of the License at
 *
 *        http://www.apache.org/licenses/LICENSE-2.0
 *
 *    Unless required by applicable law or agreed to in writing, software
 *    distributed under the License is distributed on an "AS IS" BASIS,
 *    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *    See the License for the specific language governing permissions and
 *    limitations under the License.
 */

use core::{
    cell::RefCell,
    sync::atomic::{AtomicU32, Ordering},
};

use super::objects::*;
use crate::{
    acl::{Accessor, AclMgr},
    error::*,
    interaction_model::{
        core::{Interaction, Transaction},
        messages::msg::SubscribeResp,
    },
    tlv::{TLVWriter, TagType, ToTLV},
    transport::packet::Packet,
};

// TODO: For now...
static SUBS_ID: AtomicU32 = AtomicU32::new(1);

pub struct DataModel<'a, T> {
    pub acl_mgr: &'a RefCell<AclMgr>,
    pub node: &'a Node<'a>,
    pub handler: T,
}

impl<'a, T> DataModel<'a, T> {
    pub const fn new(acl_mgr: &'a RefCell<AclMgr>, node: &'a Node<'a>, handler: T) -> Self {
        Self {
            acl_mgr,
            node,
            handler,
        }
    }

    pub fn handle(
        &mut self,
        interaction: Interaction,
        tx: &mut Packet,
        transaction: &mut Transaction,
    ) -> Result<bool, Error>
    where
        T: Handler,
    {
        let accessor = Accessor::for_session(transaction.session(), self.acl_mgr);
        let mut tw = TLVWriter::new(tx.get_writebuf()?);

        match interaction {
            Interaction::Read(req) => {
                let mut resume_path = None;

                for item in self.node.read(&req, &accessor) {
                    if let Some(path) = AttrDataEncoder::handle_read(item, &self.handler, &mut tw)?
                    {
                        resume_path = Some(path);
                        break;
                    }
                }

                req.complete(tx, transaction, resume_path)
            }
            Interaction::Write(req) => {
                for item in self.node.write(&req, &accessor) {
                    AttrDataEncoder::handle_write(item, &mut self.handler, &mut tw)?;
                }

                req.complete(tx, transaction)
            }
            Interaction::Invoke(req) => {
                for item in self.node.invoke(&req, &accessor) {
                    CmdDataEncoder::handle(item, &mut self.handler, transaction, &mut tw)?;
                }

                req.complete(tx, transaction)
            }
            Interaction::Subscribe(req) => {
                let mut resume_path = None;

                for item in self.node.subscribing_read(&req, &accessor) {
                    if let Some(path) = AttrDataEncoder::handle_read(item, &self.handler, &mut tw)?
                    {
                        resume_path = Some(path);
                        break;
                    }
                }

                req.complete(tx, transaction, resume_path)
            }
            Interaction::Timed(_) => Ok(false),
            Interaction::ResumeRead(req) => {
                let mut resume_path = None;

                for item in self.node.resume_read(&req, &accessor) {
                    if let Some(path) = AttrDataEncoder::handle_read(item, &self.handler, &mut tw)?
                    {
                        resume_path = Some(path);
                        break;
                    }
                }

                req.complete(tx, transaction, resume_path)
            }
            Interaction::ResumeSubscribe(req) => {
                let mut resume_path = None;

                if req.resume_path.is_some() {
                    for item in self.node.resume_subscribing_read(&req, &accessor) {
                        if let Some(path) =
                            AttrDataEncoder::handle_read(item, &self.handler, &mut tw)?
                        {
                            resume_path = Some(path);
                            break;
                        }
                    }
                } else {
                    // TODO
                    let resp = SubscribeResp::new(SUBS_ID.fetch_add(1, Ordering::SeqCst), 40);
                    resp.to_tlv(&mut tw, TagType::Anonymous)?;
                }

                req.complete(tx, transaction, resume_path)
            }
        }
    }

    #[cfg(feature = "nightly")]
    pub async fn handle_async<'p>(
        &mut self,
        interaction: Interaction<'_>,
        tx: &'p mut Packet<'_>,
        transaction: &mut Transaction<'_, '_>,
    ) -> Result<bool, Error>
    where
        T: super::objects::asynch::AsyncHandler,
    {
        let accessor = Accessor::for_session(transaction.session(), self.acl_mgr);
        let mut tw = TLVWriter::new(tx.get_writebuf()?);

        match interaction {
            Interaction::Read(req) => {
                let mut resume_path = None;

                for item in self.node.read(&req, &accessor) {
                    if let Some(path) =
                        AttrDataEncoder::handle_read_async(item, &self.handler, &mut tw).await?
                    {
                        resume_path = Some(path);
                        break;
                    }
                }

                req.complete(tx, transaction, resume_path)
            }
            Interaction::Write(req) => {
                for item in self.node.write(&req, &accessor) {
                    AttrDataEncoder::handle_write_async(item, &mut self.handler, &mut tw).await?;
                }

                req.complete(tx, transaction)
            }
            Interaction::Invoke(req) => {
                for item in self.node.invoke(&req, &accessor) {
                    CmdDataEncoder::handle_async(item, &mut self.handler, transaction, &mut tw)
                        .await?;
                }

                req.complete(tx, transaction)
            }
            Interaction::Subscribe(req) => {
                let mut resume_path = None;

                for item in self.node.subscribing_read(&req, &accessor) {
                    if let Some(path) =
                        AttrDataEncoder::handle_read_async(item, &self.handler, &mut tw).await?
                    {
                        resume_path = Some(path);
                        break;
                    }
                }

                req.complete(tx, transaction, resume_path)
            }
            Interaction::Timed(_) => Ok(false),
            Interaction::ResumeRead(req) => {
                let mut resume_path = None;

                for item in self.node.resume_read(&req, &accessor) {
                    if let Some(path) =
                        AttrDataEncoder::handle_read_async(item, &self.handler, &mut tw).await?
                    {
                        resume_path = Some(path);
                        break;
                    }
                }

                req.complete(tx, transaction, resume_path)
            }
            Interaction::ResumeSubscribe(req) => {
                let mut resume_path = None;

                if req.resume_path.is_some() {
                    for item in self.node.resume_subscribing_read(&req, &accessor) {
                        if let Some(path) =
                            AttrDataEncoder::handle_read_async(item, &self.handler, &mut tw).await?
                        {
                            resume_path = Some(path);
                            break;
                        }
                    }
                } else {
                    // TODO
                    let resp = SubscribeResp::new(SUBS_ID.fetch_add(1, Ordering::SeqCst), 40);
                    resp.to_tlv(&mut tw, TagType::Anonymous)?;
                }

                req.complete(tx, transaction, resume_path)
            }
        }
    }
}

pub trait DataHandler {
    fn handle(
        &mut self,
        interaction: Interaction,
        tx: &mut Packet,
        transaction: &mut Transaction,
    ) -> Result<bool, Error>;
}

impl<T> DataHandler for &mut T
where
    T: DataHandler,
{
    fn handle(
        &mut self,
        interaction: Interaction,
        tx: &mut Packet,
        transaction: &mut Transaction,
    ) -> Result<bool, Error> {
        (**self).handle(interaction, tx, transaction)
    }
}

impl<'a, T> DataHandler for DataModel<'a, T>
where
    T: Handler,
{
    fn handle(
        &mut self,
        interaction: Interaction,
        tx: &mut Packet,
        transaction: &mut Transaction,
    ) -> Result<bool, Error> {
        DataModel::handle(self, interaction, tx, transaction)
    }
}

#[cfg(feature = "nightly")]
pub mod asynch {
    use crate::{
        data_model::objects::asynch::AsyncHandler,
        error::Error,
        interaction_model::core::{Interaction, Transaction},
        transport::packet::Packet,
    };

    use super::DataModel;

    pub trait AsyncDataHandler {
        async fn handle(
            &mut self,
            interaction: Interaction<'_>,
            tx: &mut Packet,
            transaction: &mut Transaction,
        ) -> Result<bool, Error>;
    }

    impl<T> AsyncDataHandler for &mut T
    where
        T: AsyncDataHandler,
    {
        async fn handle(
            &mut self,
            interaction: Interaction<'_>,
            tx: &mut Packet<'_>,
            transaction: &mut Transaction<'_, '_>,
        ) -> Result<bool, Error> {
            (**self).handle(interaction, tx, transaction).await
        }
    }

    impl<'a, T> AsyncDataHandler for DataModel<'a, T>
    where
        T: AsyncHandler,
    {
        async fn handle(
            &mut self,
            interaction: Interaction<'_>,
            tx: &mut Packet<'_>,
            transaction: &mut Transaction<'_, '_>,
        ) -> Result<bool, Error> {
            DataModel::handle_async(self, interaction, tx, transaction).await
        }
    }
}
