//! The whole idea is to:
//! Have a way to hide things from an union to support multiple representation.

#![feature(trace_macros)]
trace_macros!(true);

use async_graphql::{
    Context, EmptyMutation, EmptySubscription, FieldResult, Object, Schema, SimpleObject, Union,
};
use warp::Filter;

fn machine_rpz<'ctx>(ctx: &'ctx Context<'_>) -> bool {
    ctx.data::<PreviewsSettings>()
        .map(|x| x.statut() == PreviewStatut::MACHINE)
        .unwrap_or(false)
}

fn human_rpz<'ctx>(ctx: &'ctx Context<'_>) -> bool {
    ctx.data::<PreviewsSettings>()
        .map(|x| x.statut() == PreviewStatut::HUMAN)
        .unwrap_or(false)
}

#[derive(PartialEq, Eq)]
enum PreviewStatut {
    MACHINE,
    HUMAN,
    NONE,
}

struct PreviewsSettings {
    pub machine_rpz: bool,
    pub human_rpz: bool,
}

impl PreviewsSettings {
    fn statut(&self) -> PreviewStatut {
        match (self.machine_rpz, self.human_rpz) {
            (true, _) => PreviewStatut::MACHINE,
            (_, true) => PreviewStatut::HUMAN,
            (_, _) => PreviewStatut::NONE,
        }
    }
}

#[derive(SimpleObject)]
struct HumanDuration {
    value_a: i32,
}

#[derive(SimpleObject)]
struct MachineDuration {
    value_b: i32,
}

#[derive(Union)]
enum Duration {
    #[graphql(visible = "human_rpz")]
    HumanDuration(HumanDuration),
    #[graphql(visible = "machine_rpz")]
    MachineDuration(MachineDuration),
}

#[derive(Default)]
pub struct Query;

#[Object]
impl Query {
    async fn test<'ctx>(&self, ctx: &'ctx Context<'_>) -> FieldResult<Duration> {
        let preview = ctx.data::<PreviewsSettings>()?;

        match preview.statut() {
            PreviewStatut::MACHINE => {
                Ok(Duration::MachineDuration(MachineDuration { value_b: 12 }))
            }
            PreviewStatut::HUMAN => Ok(Duration::HumanDuration(HumanDuration { value_a: 12 })),
            PreviewStatut::NONE => Err("Should provide machine or human rpz".into()),
        }
    }
}

const MACHINE_PREVIEW: &str = "application/vnd.company.machine";
const HUMAN_PREVIEW: &str = "application/vnd.company.human";

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let schema = Schema::build(Query::default(), EmptyMutation, EmptySubscription).finish();
    let graphql_post = warp::post()
        .and(warp::path("graphql"))
        .and(async_graphql_warp::graphql(schema))
        .and(warp::header::optional::<String>("Accept"))
        .and_then(
            |schema: (
                Schema<Query, EmptyMutation, EmptySubscription>,
                async_graphql::Request,
            ),
             preview: Option<String>| async move {
                let (schema, request) = schema;
                let headers = preview.unwrap_or("".to_string());
                println!("Accept headers: {}", headers);
                let headers = headers.split(',').collect::<Vec<&str>>();
                let preview_settings = PreviewsSettings {
                    machine_rpz: headers.contains(&MACHINE_PREVIEW),
                    human_rpz: headers.contains(&HUMAN_PREVIEW),
                };
                // Store the session inside the request
                let res = schema.execute(request.data(preview_settings)).await;

                Ok::<_, std::convert::Infallible>(async_graphql_warp::Response::from(res))
            },
        );

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
    warp::serve(graphql_post).bind(addr).await;

    Ok(())
}
