use crate::{Adapter, GraphqlApp, WebFrameworkConfig};
use juniper::GraphQLType;

use diesel::r2d2::ConnectionManager;
use futures::future;
use hyper::rt::{self, Future};
use hyper::service::service_fn;
use hyper::Method;
use hyper::Request;
use hyper::{Body, Response, Server, StatusCode};
use juniper::EmptyMutation;
use juniper::RootNode;
use r2d2::{Pool, PooledConnection};
use std::sync::Arc;

pub use hyper;

pub trait CreateContext<Connection>
where
    Self: Sized,
    Connection: 'static + diesel::Connection,
{
    fn create(
        database_connection_pool: &Pool<ConnectionManager<Connection>>,
        request: &Request<Body>,
    ) -> Result<Self, Box<dyn std::error::Error>>;
}

pub struct HyperAdapter {
    _unit: (),
}

impl<Connection, Query, Mutation, Context> Adapter<Connection, Query, Mutation, Context>
    for HyperAdapter
where
    Connection: 'static + diesel::Connection,
    Query: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Mutation: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Context: 'static + Send + Sync + juniper::Context + CreateContext<Connection>,
{
    type Inner = ();

    fn new() -> Self {
        HyperAdapter { _unit: () }
    }

    fn run<App>(&self, app: App, config: WebFrameworkConfig<Connection>)
    where
        App: GraphqlApp<
            Adapter = Self,
            Connection = Connection,
            Query = Query,
            Mutation = Mutation,
            Context = Context,
        >,
    {
        let WebFrameworkConfig {
            database_connection_pool,
            graphql_path,
            mount_graphiql_at,
            mount_graphql_at,
            port,
        } = config;

        let addr = ([127, 0, 0, 1], port).into();

        let root_node = Arc::new(RootNode::new(Query::default(), Mutation::default()));

        let new_service = move || {
            let root_node = root_node.clone();
            let database_connection_pool = database_connection_pool.clone();
            service_fn(move |req| -> Box<dyn Future<Item = _, Error = _> + Send> {
                let root_node = root_node.clone();

                let ctx =
                    <Context as CreateContext<Connection>>::create(&database_connection_pool, &req);

                match ctx {
                    Ok(ctx) => {
                        let ctx = Arc::new(ctx);

                        match (req.method(), req.uri().path()) {
                            (&Method::GET, "/") => Box::new(juniper_hyper::graphiql("/graphql")),
                            (&Method::GET, "/graphql") => {
                                Box::new(juniper_hyper::graphql(root_node, ctx, req))
                            }
                            (&Method::POST, "/graphql") => {
                                Box::new(juniper_hyper::graphql(root_node, ctx, req))
                            }
                            _ => {
                                let mut response = Response::new(Body::empty());
                                *response.status_mut() = StatusCode::NOT_FOUND;
                                Box::new(future::ok(response))
                            }
                        }
                    }
                    Err(err) => {
                        let err_msg = err.to_string();
                        let mut response = Response::new(Body::from(err_msg));
                        *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
                        Box::new(future::ok(response))
                    }
                }
            })
        };

        let server = Server::bind(&addr)
            .serve(new_service)
            .map_err(|e| panic!("server error: {}", e));

        println!("Listening on http://{}", addr);

        rt::run(server);
    }
}
