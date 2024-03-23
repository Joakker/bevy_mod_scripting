use log::{info, trace};
use rustc_errors::FatalError;
use rustc_hir::def_id::LOCAL_CRATE;

use crate::{Args, ALL_PASSES};

pub(crate) struct BevyAnalyzerCallbacks {
    args: Args,
}

impl BevyAnalyzerCallbacks {
    pub(crate) fn new(args: Args) -> Self {
        Self { args }
    }
}

impl rustc_driver::Callbacks for BevyAnalyzerCallbacks {
    fn after_expansion<'tcx>(
        &mut self,
        compiler: &rustc_interface::interface::Compiler,
        queries: &'tcx rustc_interface::Queries<'tcx>,
    ) -> rustc_driver::Compilation {
        trace!("After expansion callback");
        let Ok(mut gcx) = queries.global_ctxt() else {
            FatalError.raise()
        };
        let sess = &compiler.sess;

        if sess.dcx().has_errors().is_some() {
            sess.dcx().fatal("compilation failed, aborting analysis.");
        }

        let mut meta_dirs = Vec::default();
        // add all relevant meta dirs to the context
        if let crate::Command::Generate {
            output,
            meta,
            meta_output,
            ..
        } = &self.args.cmd
        {
            meta_dirs.push(output.to_owned());
            meta.iter()
                .flatten()
                .chain(meta_output.iter())
                .for_each(|m| meta_dirs.push(m.to_owned()));
        };

        gcx.enter(|tcx| {
            let mut ctxt = crate::BevyCtxt::new(tcx, meta_dirs);

            trace!("Running all passes");
            for p in ALL_PASSES {
                info!(
                    "Running pass: '{}' on crate: '{}'",
                    p.name,
                    tcx.crate_name(LOCAL_CRATE)
                );
                let continue_ = tcx.sess.time(p.name, || (p.cb)(&mut ctxt, &self.args));
                if !continue_ {
                    info!(
                        "Pass: '{}' requested compilation to stop, stopping early",
                        p.name
                    );
                    break;
                }
            }
        });
        rustc_driver::Compilation::Continue
    }
}