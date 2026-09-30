use std::{
    alloc::{self, Layout},
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Stored at the start of every entry's allocation, directly followed by the data
#[repr(C)]
struct Header {
    /// Number of live [Interned](crate::Interned) handles, the pool itself isn't counted
    handles: AtomicUsize,
    /// Cached hash of the data, so it never has to be recomputed on drop or resize
    hash: u64,
    len: usize,
}

/// The data always starts right after the header, since `u8` needs no alignment
const DATA_OFFSET: usize = size_of::<Header>();

/// A pointer to an entry's allocation, which is owned by the pool
///
/// The pool frees the allocation once no handles are left, so every access requires that either
/// a handle is alive or the shard holding the entry is locked
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub(crate) struct Entry(NonNull<Header>);

// Safety: the data is immutable apart from the atomic handle count
unsafe impl Send for Entry {}
unsafe impl Sync for Entry {}

impl Entry {
    fn layout(len: usize) -> Layout {
        Layout::from_size_align(DATA_OFFSET + len, align_of::<Header>())
            .expect("interned value is too large")
    }

    /// Allocates a new entry with no handles
    pub(crate) fn new(value: &[u8], hash: u64) -> Self {
        let layout = Self::layout(value.len());

        // Safety: the layout is never zero-sized since it includes the header
        let ptr = unsafe { alloc::alloc(layout) };
        let Some(ptr) = NonNull::new(ptr) else {
            alloc::handle_alloc_error(layout);
        };

        let header = Header {
            handles: AtomicUsize::new(0),
            hash,
            len: value.len(),
        };

        // Safety: the allocation fits the header followed by `value.len()` bytes
        unsafe {
            ptr.cast::<Header>().write(header);
            ptr.add(DATA_OFFSET)
                .copy_from_nonoverlapping(NonNull::from(value).cast(), value.len());
        }

        Self(ptr.cast())
    }

    /// Frees the entry's allocation
    ///
    /// # Safety
    ///
    /// The entry must have been removed from the pool, with no handles left
    pub(crate) unsafe fn free(self) {
        // Safety: guaranteed by the caller
        unsafe {
            let layout = Self::layout(self.header().len);
            alloc::dealloc(self.0.as_ptr().cast(), layout);
        }
    }

    /// # Safety
    ///
    /// The entry must be alive, see [Entry]
    unsafe fn header(&self) -> &Header {
        // Safety: guaranteed by the caller
        unsafe { self.0.as_ref() }
    }

    /// # Safety
    ///
    /// The entry must be alive, see [Entry]
    pub(crate) unsafe fn hash(&self) -> u64 {
        // Safety: guaranteed by the caller
        unsafe { self.header().hash }
    }

    /// The address of the data, which is also what identifies the entry
    ///
    /// Doesn't access the entry, so it's fine to call even if it was freed
    pub(crate) fn data_ptr(self) -> *const u8 {
        self.0.cast::<u8>().as_ptr().wrapping_add(DATA_OFFSET)
    }

    /// # Safety
    ///
    /// The entry must stay alive for `'a`, see [Entry]
    pub(crate) unsafe fn data<'a>(self) -> &'a [u8] {
        // Safety: guaranteed by the caller, and the data was initialized in [Entry::new]
        unsafe { std::slice::from_raw_parts(self.data_ptr(), self.header().len) }
    }

    /// Registers a new handle
    ///
    /// # Safety
    ///
    /// The entry must be alive, see [Entry]
    pub(crate) unsafe fn acquire(self) -> Self {
        // same limit as std's Arc, aborting since a wrapped count would lead to use-after-free
        const MAX_HANDLES: usize = isize::MAX as usize;

        // Safety: guaranteed by the caller
        if unsafe { self.header() }
            .handles
            .fetch_add(1, Ordering::Relaxed)
            > MAX_HANDLES
        {
            std::process::abort();
        }

        self
    }

    /// Gives up a handle, returning whether it was the last one
    ///
    /// The entry may be freed by another thread as soon as this returns, so it must not be
    /// accessed afterwards without locking its shard
    ///
    /// # Safety
    ///
    /// The caller must own the handle being given up
    pub(crate) unsafe fn release(self) -> bool {
        // Safety: guaranteed by the caller
        unsafe { self.header() }
            .handles
            .fetch_sub(1, Ordering::Release)
            == 1
    }

    /// Whether no handles are left
    ///
    /// # Safety
    ///
    /// The shard holding the entry must be locked
    pub(crate) unsafe fn is_unused(&self) -> bool {
        // Safety: guaranteed by the caller
        unsafe { self.header() }.handles.load(Ordering::Acquire) == 0
    }
}
