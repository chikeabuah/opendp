use std::convert::TryFrom;
use std::os::raw::{c_char, c_void};

use crate::core::{FfiResult, IntoAnyMeasurementFfiResultExt, Measure, MetricSpace};
use crate::domains::{AtomDomain, VectorDomain};
use crate::error::Fallible;
use crate::ffi::any::{AnyDomain, AnyMeasurement, AnyMetric, Downcast};
use crate::ffi::util::Type;
use crate::measurements::{make_gaussian, BaseDiscreteGaussianDomain, MakeGaussian};
use crate::measures::ZeroConcentratedDivergence;
use crate::traits::{CheckAtom, InfCast, Number};

#[no_mangle]
pub extern "C" fn opendp_measurements__make_gaussian(
    input_domain: *const AnyDomain,
    input_metric: *const AnyMetric,
    scale: *const c_void,
    MO: *const c_char,
) -> FfiResult<*mut AnyMeasurement> {
    fn monomorphize_float<T: 'static + CheckAtom + Copy>(
        input_domain: &AnyDomain,
        input_metric: &AnyMetric,
        scale: *const c_void,
        MO: Type,
    ) -> Fallible<AnyMeasurement>
    where
        AtomDomain<T>: MakeGaussian<ZeroConcentratedDivergence<T>, T>,
        VectorDomain<AtomDomain<T>>: MakeGaussian<ZeroConcentratedDivergence<T>, T>,
        (
            AtomDomain<T>,
            <AtomDomain<T> as BaseDiscreteGaussianDomain<T>>::InputMetric,
        ): MetricSpace,
        (
            VectorDomain<AtomDomain<T>>,
            <VectorDomain<AtomDomain<T>> as BaseDiscreteGaussianDomain<T>>::InputMetric,
        ): MetricSpace,
    {
        fn monomorphize2<D: 'static + MakeGaussian<MO, MO::Distance>, MO: 'static + Measure>(
            input_domain: &AnyDomain,
            input_metric: &AnyMetric,
            scale: MO::Distance,
        ) -> Fallible<AnyMeasurement>
        where
            (D, D::InputMetric): MetricSpace,
        {
            let input_domain = input_domain.downcast_ref::<D>()?.clone();
            let input_metric = input_metric.downcast_ref::<D::InputMetric>()?.clone();
            make_gaussian::<D, MO, MO::Distance>(input_domain, input_metric, scale).into_any()
        }
        let D = input_domain.type_.clone();
        let scale = *try_as_ref!(scale as *const T);
        dispatch!(monomorphize2, [
            (D, [AtomDomain<T>, VectorDomain<AtomDomain<T>>]),
            (MO, [ZeroConcentratedDivergence<T>])
        ], (input_domain, input_metric, scale))
    }
    fn monomorphize_integer<
        T: 'static + CheckAtom,
        QI: 'static + Copy,
        QO: 'static + Number + InfCast<QI>,
    >(
        input_domain: &AnyDomain,
        input_metric: &AnyMetric,
        scale: *const c_void,
        MO: Type,
        QI: Type,
    ) -> Fallible<AnyMeasurement>
    where
        AtomDomain<T>: MakeGaussian<ZeroConcentratedDivergence<QO>, QI>,
        VectorDomain<AtomDomain<T>>: MakeGaussian<ZeroConcentratedDivergence<QO>, QI>,
        (
            AtomDomain<T>,
            <AtomDomain<T> as BaseDiscreteGaussianDomain<QI>>::InputMetric,
        ): MetricSpace,
        (
            VectorDomain<AtomDomain<T>>,
            <VectorDomain<AtomDomain<T>> as BaseDiscreteGaussianDomain<QI>>::InputMetric,
        ): MetricSpace,
    {
        fn monomorphize2<
            D: 'static + MakeGaussian<MO, QI>,
            MO: 'static + Measure,
            QI: 'static + Copy,
        >(
            input_domain: &AnyDomain,
            input_metric: &AnyMetric,
            scale: MO::Distance,
        ) -> Fallible<AnyMeasurement>
        where
            MO::Distance: Number + InfCast<QI>,
            (D, D::InputMetric): MetricSpace,
        {
            let input_domain = input_domain.downcast_ref::<D>()?.clone();
            let input_metric = input_metric.downcast_ref::<D::InputMetric>()?.clone();
            make_gaussian::<D, MO, QI>(input_domain, input_metric, scale).into_any()
        }
        let D = input_domain.type_.clone();
        let scale = *try_as_ref!(scale as *const QO);
        dispatch!(monomorphize2, [
            (D, [AtomDomain<T>, VectorDomain<AtomDomain<T>>]),
            (MO, [ZeroConcentratedDivergence<QO>]),
            (QI, [QI])
        ], (input_domain, input_metric, scale))
    }
    let input_domain = try_as_ref!(input_domain);
    let input_metric = try_as_ref!(input_metric);
    let T = try_!(input_domain.type_.get_atom());
    let MO = try_!(Type::try_from(MO));
    let QO = try_!(MO.get_atom());

    // This is used to check if the type is in a dispatch set,
    // without constructing an expensive backtrace upon failed match
    fn in_set<T>() -> Option<()> {
        Some(())
    }

    if let Some(_) = dispatch!(in_set, [(T, @floats)]) {
        let QI = try_!(input_metric.distance_type.get_atom());
        if T != QI {
            return err!(
                FFI,
                "since data type is float, input distance type ({}) must match data type ({})",
                QI.descriptor,
                T.descriptor
            )
            .into();
        }
        if T != QO {
            return err!(
                FFI,
                "since data type is float, output distance type ({}) must match data type ({})",
                QO.descriptor,
                T.descriptor
            )
            .into();
        }
        dispatch!(monomorphize_float, [
            (T, @floats)
        ], (input_domain, input_metric, scale, MO))
    } else {
        let QI = input_metric.distance_type.clone();
        dispatch!(monomorphize_integer, [
            (T, @integers),
            (QI, @numbers),
            (QO, @floats)
        ], (input_domain, input_metric, scale, MO, QI))
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core;
    use crate::error::Fallible;
    use crate::ffi::any::{AnyObject, Downcast};
    use crate::ffi::util;
    use crate::ffi::util::ToCharP;
    use crate::metrics::AbsoluteDistance;

    #[test]
    fn test_make_gaussian_ffi() -> Fallible<()> {
        let measurement = Result::from(opendp_measurements__make_gaussian(
            util::into_raw(AnyDomain::new(AtomDomain::<i32>::default())),
            util::into_raw(AnyMetric::new(AbsoluteDistance::<i32>::default())),
            util::into_raw(0.0) as *const c_void,
            "ZeroConcentratedDivergence<f64>".to_char_p(),
        ))?;
        let arg = AnyObject::new_raw(99);
        let res = core::opendp_core__measurement_invoke(&measurement, arg);
        let res: i32 = Fallible::from(res)?.downcast()?;
        assert_eq!(res, 99);
        Ok(())
    }
}
