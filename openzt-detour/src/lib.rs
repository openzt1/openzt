use std::marker::PhantomData;

use retour::GenericDetour;

pub mod generated;

pub struct FunctionDef<T> {
    pub address: u32,
    function_type: PhantomData<T>,
}

impl<T> FunctionDef<T>
where
    T: retour::Function,
{
    /// # Safety
    ///
    /// This function will cause issues if the address or signature is not correct.
    pub unsafe fn detour(self, target: T) -> Result<GenericDetour<T>, retour::Error> {
        unsafe { GenericDetour::<T>::new(::retour::Function::from_ptr(self.address as *const ()), target) }
    }

    // TODO: Would be nice to have a `call` that calls the original function without having to detour it first.
    /// # Safety
    ///
    /// This function will cause issues if the address is not correct
    pub unsafe fn original(&self) -> T {
        unsafe { ::retour::Function::from_ptr(self.address as *const ()) }
    }
}
