pub use crate::label::ComposedGroup;

use super::{
    name::{MetricNameEncoder, WithNamespace},
    MetricEncoding,
};

pub trait Encoding {
    /// Write the help text for a metric
    fn write_help(&mut self, name: impl MetricNameEncoder, help: &str);
}

impl<E: Encoding> Encoding for &mut E {
    fn write_help(&mut self, name: impl MetricNameEncoder, help: &str) {
        E::write_help(self, name, help);
    }
}

pub trait MetricGroup<Enc: Encoding> {
    fn collect_into(&self, enc: &mut Enc);
}

impl<G, E> MetricGroup<E> for &G
where
    G: MetricGroup<E>,
    E: Encoding,
{
    fn collect_into(&self, enc: &mut E) {
        G::collect_into(self, enc);
    }
}

impl<A, B, E> MetricGroup<E> for ComposedGroup<A, B>
where
    A: MetricGroup<E>,
    B: MetricGroup<E>,
    E: Encoding,
{
    fn collect_into(&self, enc: &mut E) {
        self.0.collect_into(enc);
        self.1.collect_into(enc);
    }
}

impl<G, E> MetricGroup<E> for WithNamespace<G>
where
    G: for<'a> MetricGroup<WithNamespace<&'a mut E>>,
    E: Encoding,
{
    fn collect_into(&self, enc: &mut E) {
        self.inner.collect_into(&mut WithNamespace {
            namespace: self.namespace,
            inner: enc,
        });
    }
}

impl<E: Encoding> Encoding for WithNamespace<E> {
    fn write_help(&mut self, name: impl MetricNameEncoder, help: &str) {
        self.inner.write_help(
            WithNamespace {
                namespace: self.namespace,
                inner: name,
            },
            help,
        )
    }
}

impl<M: MetricEncoding<E>, E> MetricEncoding<WithNamespace<E>> for M {
    fn write_type(name: impl MetricNameEncoder, enc: &mut WithNamespace<E>) {
        M::write_type(
            WithNamespace {
                namespace: enc.namespace,
                inner: name,
            },
            &mut enc.inner,
        );
    }
    fn collect_into(
        &self,
        metadata: &M::Metadata,
        labels: impl crate::label::LabelGroup,
        name: impl MetricNameEncoder,
        enc: &mut WithNamespace<E>,
    ) {
        self.collect_into(
            metadata,
            labels,
            WithNamespace {
                namespace: enc.namespace,
                inner: name,
            },
            &mut enc.inner,
        )
    }
}

impl<'a, M: MetricEncoding<E>, E> MetricEncoding<&'a mut E> for M {
    fn write_type(name: impl MetricNameEncoder, enc: &mut &'a mut E) {
        M::write_type(name, *enc);
    }
    fn collect_into(
        &self,
        metadata: &M::Metadata,
        labels: impl crate::label::LabelGroup,
        name: impl MetricNameEncoder,
        enc: &mut &'a mut E,
    ) {
        self.collect_into(metadata, labels, name, *enc)
    }
}

#[cfg(all(feature = "lasso", test))]
mod tests {
    use lasso::{Rodeo, RodeoReader};
    use measured_derive::{FixedCardinalityLabel, LabelGroup, MetricGroup};
    use prometheus_client::encoding::EncodeLabelValue;

    use crate::{label::StaticLabelSet, text::TextEncoder, Counter, CounterVec};

    use super::MetricGroup;

    #[derive(Clone, Copy, PartialEq, Debug, LabelGroup)]
    #[label(crate = crate, set = ErrorsSet)]
    struct Error<'a> {
        kind: ErrorKind,
        #[label(fixed_with = RodeoReader)]
        route: &'a str,
    }

    #[derive(Clone, Copy, PartialEq, Debug, Hash, Eq, FixedCardinalityLabel, EncodeLabelValue)]
    #[label(crate = crate)]
    enum ErrorKind {
        User,
        Internal,
        Network,
    }

    #[derive(MetricGroup)]
    #[metric(crate = crate)]
    #[metric(new(route: RodeoReader))]
    struct MyHttpMetrics {
        /// more help wow
        #[metric(init = CounterVec::new(route))]
        errors: CounterVec<ErrorsSet>,
    }

    #[derive(MetricGroup)]
    #[metric(crate = crate)]
    #[metric(new(route: RodeoReader))]
    struct MyMetrics {
        // implici #[metric(default)]
        /// help text
        events_total: Counter,

        #[metric(namespace = "http_request")]
        #[metric(init = MyHttpMetrics::new(route))]
        http: MyHttpMetrics,
    }

    #[test]
    fn http_errors() {
        let group = MyMetrics {
            events_total: Counter::new(),
            http: MyHttpMetrics {
                errors: CounterVec::new(ErrorsSet {
                    kind: StaticLabelSet::new(),
                    route: Rodeo::from_iter([
                        "/api/v1/users",
                        "/api/v1/users/:id",
                        "/api/v1/products",
                        "/api/v1/products/:id",
                        "/api/v1/products/:id/owner",
                        "/api/v1/products/:id/purchase",
                    ])
                    .into_reader(),
                }),
            },
        };

        let mut text_encoder = TextEncoder::new();
        group.collect_into(&mut text_encoder);
        assert_eq!(
            text_encoder.finish(),
            r#"# HELP events_total help text
# TYPE events_total counter
events_total 0

# HELP http_request_errors more help wow
# TYPE http_request_errors counter
http_request_errors{kind="user",route="/api/v1/users"} 0
http_request_errors{kind="user",route="/api/v1/users/:id"} 0
http_request_errors{kind="user",route="/api/v1/products"} 0
http_request_errors{kind="user",route="/api/v1/products/:id"} 0
http_request_errors{kind="user",route="/api/v1/products/:id/owner"} 0
http_request_errors{kind="user",route="/api/v1/products/:id/purchase"} 0
http_request_errors{kind="internal",route="/api/v1/users"} 0
http_request_errors{kind="internal",route="/api/v1/users/:id"} 0
http_request_errors{kind="internal",route="/api/v1/products"} 0
http_request_errors{kind="internal",route="/api/v1/products/:id"} 0
http_request_errors{kind="internal",route="/api/v1/products/:id/owner"} 0
http_request_errors{kind="internal",route="/api/v1/products/:id/purchase"} 0
http_request_errors{kind="network",route="/api/v1/users"} 0
http_request_errors{kind="network",route="/api/v1/users/:id"} 0
http_request_errors{kind="network",route="/api/v1/products"} 0
http_request_errors{kind="network",route="/api/v1/products/:id"} 0
http_request_errors{kind="network",route="/api/v1/products/:id/owner"} 0
http_request_errors{kind="network",route="/api/v1/products/:id/purchase"} 0
"#
        );
    }
}
