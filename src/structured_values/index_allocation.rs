// Copyright 2021 Colin Finck <colin@reactos.org>
// SPDX-License-Identifier: GPL-2.0-or-later

use crate::attribute::NtfsAttributeType;
use crate::error::{NtfsError, Result};
use crate::index_record::NtfsIndexRecord;
use crate::structured_values::index_root::NtfsIndexRoot;
use crate::structured_values::{
    NtfsStructuredValue, NtfsStructuredValueFromNonResidentAttributeValue,
};
use crate::traits::NtfsReadSeek;
use crate::types::Vcn;
use crate::value::non_resident_attribute::NtfsNonResidentAttributeValue;
use binread::io::{Read, Seek, SeekFrom};
use core::iter::FusedIterator;

#[derive(Clone, Debug)]
pub struct NtfsIndexAllocation<'n, 'f> {
    value: NtfsNonResidentAttributeValue<'n, 'f>,
}

impl<'n, 'f> NtfsIndexAllocation<'n, 'f> {
    pub fn iter(&self, index_root: &NtfsIndexRoot) -> NtfsIndexRecords<'n, 'f> {
        let index_record_size = index_root.index_record_size();
        NtfsIndexRecords::new(self.value.clone(), index_record_size)
    }

    pub fn record_from_vcn<T>(
        &self,
        fs: &mut T,
        index_root: &NtfsIndexRoot,
        vcn: Vcn,
    ) -> Result<NtfsIndexRecord<'n>>
    where
        T: Read + Seek,
    {
        // Seek to the byte offset of the given VCN.
        let mut value = self.value.clone();
        let offset = vcn.offset(self.value.ntfs())?;
        value.seek(fs, SeekFrom::Current(offset))?;

        if value.stream_position() >= value.len() {
            return Err(NtfsError::VcnOutOfBoundsInIndexAllocation {
                position: self.value.position(),
                vcn,
            });
        }

        // Get the record.
        let index_record_size = index_root.index_record_size();
        let record = NtfsIndexRecord::new(fs, value, index_record_size)?;

        // Validate that the VCN in the record is the requested one.
        if record.vcn() != vcn {
            return Err(NtfsError::VcnMismatchInIndexAllocation {
                position: self.value.position(),
                expected: vcn,
                actual: record.vcn(),
            });
        }

        Ok(record)
    }
}

impl<'n, 'f> NtfsStructuredValue for NtfsIndexAllocation<'n, 'f> {
    const TY: NtfsAttributeType = NtfsAttributeType::IndexAllocation;
}

impl<'n, 'f> NtfsStructuredValueFromNonResidentAttributeValue<'n, 'f>
    for NtfsIndexAllocation<'n, 'f>
{
    fn from_non_resident_attribute_value<T>(
        _fs: &mut T,
        value: NtfsNonResidentAttributeValue<'n, 'f>,
    ) -> Result<Self>
    where
        T: Read + Seek,
    {
        Ok(Self { value })
    }
}

#[derive(Clone, Debug)]
pub struct NtfsIndexRecords<'n, 'f> {
    value: NtfsNonResidentAttributeValue<'n, 'f>,
    index_record_size: u32,
}

impl<'n, 'f> NtfsIndexRecords<'n, 'f> {
    fn new(value: NtfsNonResidentAttributeValue<'n, 'f>, index_record_size: u32) -> Self {
        Self {
            value,
            index_record_size,
        }
    }

    pub fn attach<'a, T>(self, fs: &'a mut T) -> NtfsIndexRecordsAttached<'n, 'f, 'a, T>
    where
        T: Read + Seek,
    {
        NtfsIndexRecordsAttached::new(fs, self)
    }

    pub fn next<T>(&mut self, fs: &mut T) -> Option<Result<NtfsIndexRecord<'n>>>
    where
        T: Read + Seek,
    {
        if self.value.stream_position() >= self.value.len() {
            return None;
        }

        // Get the current record.
        let record = iter_try!(NtfsIndexRecord::new(
            fs,
            self.value.clone(),
            self.index_record_size
        ));

        // Advance our iterator to the next record.
        iter_try!(self
            .value
            .seek(fs, SeekFrom::Current(self.index_record_size as i64)));

        Some(Ok(record))
    }
}

pub struct NtfsIndexRecordsAttached<'n, 'f, 'a, T>
where
    T: Read + Seek,
{
    fs: &'a mut T,
    index_records: NtfsIndexRecords<'n, 'f>,
}

impl<'n, 'f, 'a, T> NtfsIndexRecordsAttached<'n, 'f, 'a, T>
where
    T: Read + Seek,
{
    fn new(fs: &'a mut T, index_records: NtfsIndexRecords<'n, 'f>) -> Self {
        Self { fs, index_records }
    }

    pub fn detach(self) -> NtfsIndexRecords<'n, 'f> {
        self.index_records
    }
}

impl<'n, 'f, 'a, T> Iterator for NtfsIndexRecordsAttached<'n, 'f, 'a, T>
where
    T: Read + Seek,
{
    type Item = Result<NtfsIndexRecord<'n>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.index_records.next(self.fs)
    }
}

impl<'n, 'f, 'a, T> FusedIterator for NtfsIndexRecordsAttached<'n, 'f, 'a, T> where T: Read + Seek {}
