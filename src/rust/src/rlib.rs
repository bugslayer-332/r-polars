use crate::lazy::dsl::Expr;
use crate::rdataframe::DataFrame;
use crate::{rdataframe::VecDataFrame, utils::r_result_list};

use crate::lazy::dsl::ProtoExprArray;
use crate::series::Series;
use crate::rdatatype::robj_to_timeunit;
use crate::robj_to;
use extendr_api::prelude::*;
use polars::prelude as pl;
use polars_core::functions as pl_functions;
use std::result::Result;
#[extendr]
fn concat_df(vdf: &VecDataFrame) -> List {
    //-> PyResult<PyDataFrame> {

    use polars_core::error::PolarsResult;
    use polars_core::utils::rayon::prelude::*;

    let first = (*vdf.0.iter().peekable().peek().unwrap()).clone();
    let iter = vdf.0.clone().into_iter().map(|df| df);
    let identity_df = first.clone().slice(0, 0);
    let rdfs: Vec<pl::PolarsResult<pl::DataFrame>> = iter.map(|df| Ok(df)).collect();
    let identity = || Ok(identity_df.clone());

    let result = polars_core::POOL
        .install(|| {
            rdfs.into_par_iter()
                .fold(identity, |acc: PolarsResult<pl::DataFrame>, df| {
                    let mut acc = acc?;
                    acc.vstack_mut(&df?)?;
                    Ok(acc)
                })
                .reduce(identity, |acc, df| {
                    let mut acc = acc?;
                    acc.vstack_mut(&df?)?;
                    Ok(acc)
                })
        })
        .map(|ok| DataFrame(ok));

    r_result_list(result.map_err(|err| format!("{:?}", err)))
}
//ping

#[extendr]
fn diag_concat_df(dfs: &VecDataFrame) -> List {
    let df = pl_functions::diag_concat_df(&dfs.0[..]).map(|ok| DataFrame(ok));
    r_result_list(df.map_err(|err| format!("{:?}", err)))
}

#[extendr]
pub fn hor_concat_df(dfs: &VecDataFrame) -> List {
    let df = pl_functions::hor_concat_df(&dfs.0[..]).map(|ok| DataFrame(ok));
    r_result_list(df.map_err(|err| format!("{:?}", err)))
}

#[extendr]
pub fn mem_address(robj: Robj) -> String {
    let ptr_raw = unsafe { robj.external_ptr_addr::<usize>() };
    let ptr_val = ptr_raw as usize;
    format!("{:#012x}", ptr_val)
}

#[extendr]
fn min_exprs(exprs: &ProtoExprArray) -> Expr {
    let exprs = exprs.to_vec("select");
    polars::lazy::dsl::min_exprs(exprs).into()
}

#[extendr]
fn max_exprs(exprs: &ProtoExprArray) -> Expr {
    let exprs = exprs.to_vec("select");
    polars::lazy::dsl::max_exprs(exprs).into()
}

#[extendr]
fn coalesce_exprs(exprs: &ProtoExprArray) -> Expr {
    let exprs = exprs.to_vec("select");
    polars::lazy::dsl::coalesce(&exprs).into()
}

#[extendr]
fn sum_exprs(exprs: &ProtoExprArray) -> Expr {
    let exprs = exprs.to_vec("select");
    polars::lazy::dsl::sum_exprs(exprs).into()
}

#[extendr]
fn concat_lst(exprs: &ProtoExprArray) -> Expr {
    let exprs = exprs.to_vec("select");
    polars::lazy::dsl::concat_lst(exprs).into()
}

#[extendr]
fn r_date_range(
    start: f64,
    stop: f64,
    every: &str,
    closed: &str, //Wap<ClosedWindow>
    name: &str,
    tu: Robj,
    tz: Nullable<String>,
) -> List {
    use crate::rdatatype::new_closed_window;
    use crate::utils::try_f64_into_i64;

    use pl::IntoSeries;

    let res = || -> std::result::Result<Series, String> {
        Ok(Series(
            polars::time::date_range_impl(
                name,
                try_f64_into_i64(start)?,
                try_f64_into_i64(stop)?,
                pl::Duration::parse(every),
                new_closed_window(closed)?,
                robj_to_timeunit(tu)?,
                tz.into_option().as_ref(),
            )
            .map_err(|err| format!("in r_date_range: {}", err))?
            .into_series(),
        ))
    }();
    r_result_list(res)
}

#[extendr]
fn r_date_range_lazy(
    start: &Expr,
    end: &Expr,
    every: &str,
    closed: &str,
    name: String,
    tz: Nullable<String>,
) -> List {
    use crate::rdatatype::new_closed_window;
    let res = || -> std::result::Result<Expr, String> {
        Ok(Expr(polars::lazy::dsl::functions::date_range(
            name,
            start.0.clone(),
            end.0.clone(),
            pl::Duration::parse(every),
            new_closed_window(closed)?,
            tz.into_option(),
        )))
    }();
    r_result_list(res)
}

//TODO py-polars have some fancy transmute conversions TOExprs trait, maybe imple that too
//for now just use inner directly
#[extendr]
fn as_struct(exprs: Robj) -> Result<Expr, String> {
    Ok(
        polars::lazy::dsl::as_struct(crate::utils::list_expr_to_vec_pl_expr(exprs)?.as_slice())
            .into(),
    )
}

#[extendr]
fn struct_(exprs: Robj, eager: Robj, schema: Robj) -> Result<Robj, String> {
    use crate::rdatatype::RPolarsDataType;
    let struct_expr = as_struct(exprs)?;
    let eager = robj_to!(bool, eager)?;

    let struct_expr = if !schema.is_null() {
        let schema: Vec<RPolarsDataType> = robj_to!(Vec, RPolarsDataType, schema)?;
        dbg!(&schema);
        todo!()
    } else {
        struct_expr
    };

    if eager {
        use pl::*;
        let df = pl::DataFrame::empty()
            .lazy()
            .select(&[struct_expr.0])
            .collect()
            .map_err(|err| format!("during eager evaluation of struct: {}", err))?;
        Ok(crate::rdataframe::DataFrame(df).into())
    } else {
        Ok(struct_expr.into())
    }
}

extendr_module! {
    mod rlib;
    fn concat_df;
    fn hor_concat_df;
    fn diag_concat_df;
    fn min_exprs;
    fn max_exprs;
    fn coalesce_exprs;
    fn sum_exprs;
    fn mem_address;
    fn concat_lst;
    fn r_date_range;
    fn r_date_range_lazy;
    fn as_struct;
    fn struct_;
}
