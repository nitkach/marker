use crate::ffi::FfiSlice;

use super::{CommonExprData, ExprKind};

/// A [call expression](https://doc.rust-lang.org/reference/expressions/call-expr.html#call-expressions)
/// calling a function. The called function is identified using an expression,
/// called *operand*. The following shows a few examples of call expressions:
/// ```
/// # pub fn foo(_: u32) {}
/// //  vvv The operand pointing to a function called `foo`
///     foo(1);
/// //      ^ A number literal as an argument
///
/// # let _: Vec<u32> =
///     Vec::new();
/// //  ^^^^^^^^ The operand pointing to the associated function `new()` for
/// //           the type `Vec<_>`. This is not a method call, as the function
/// //           doesn't take `self` as an argument.
///
///     (|| "Closures are cool")();
/// //  ^^^^^^^^^^^^^^^^^^^^^^^^ A closure expression as an operand
/// ```
#[repr(C)]
#[derive(Debug)]
pub struct CallExpr<'ast> {
    data: CommonExprData<'ast>,
    operand: ExprKind<'ast>,
    args: FfiSlice<'ast, ExprKind<'ast>>,
}

impl<'ast> CallExpr<'ast> {
    /// The expression identifying what will be called. This will often be a
    /// [`PathExpr`](super::PathExpr).
    pub fn operand(&self) -> ExprKind<'ast> {
        self.operand
    }

    /// The arguments given to the operand.
    pub fn args(&self) -> &[ExprKind<'ast>] {
        self.args.get()
    }
}

super::impl_expr_data!(CallExpr<'ast>, Call);

#[cfg(feature = "driver-api")]
impl<'ast> CallExpr<'ast> {
    pub fn new(data: CommonExprData<'ast>, operand: ExprKind<'ast>, args: &'ast [ExprKind<'ast>]) -> Self {
        Self {
            data,
            operand,
            args: args.into(),
        }
    }
}
