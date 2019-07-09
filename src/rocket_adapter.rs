use crate::{GraphqlApp, Adapter, WebFrameworkConfig};
use juniper::{GraphQLType, RootNode};
use juniper_rocket::GraphQLRequest;
use rocket::{
    data::{FromData, Transform},
    handler::{self, Handler},
    http::Method,
    request::{FromFormValue, FromRequest, Request},
    Data, Outcome, Route, State,
};
use std::borrow::Borrow;
use std::marker::PhantomData;

pub use rocket;

pub struct RocketAdapter {
    _unit: (),
}

impl<Connection, Query, Mutation, Context> Adapter<Connection, Query, Mutation, Context>
    for RocketAdapter
where
    Connection: 'static + diesel::Connection,
    Query: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Mutation: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Context: 'static + juniper::Context + for<'ca, 'cr> FromRequest<'ca, 'cr>,
{
    type Inner = rocket::Rocket;

    fn new() -> Self {
        RocketAdapter { _unit: () }
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
        } = config;

        let rocket = rocket::ignite()
            .manage(database_connection_pool)
            .manage(juniper::RootNode::new(
                Query::default(),
                Mutation::default(),
            ))
            .mount(mount_graphiql_at, GraphiqlHandler::new(&graphql_path))
            .mount(
                mount_graphql_at,
                PostGraphqlHandler::<Query, Mutation, Context>::new(),
            )
            .mount(
                mount_graphql_at,
                GetGraphqlHandler::<Query, Mutation, Context>::new(),
            );
        let rocket = app.configure_web_framework(rocket);

        let error = rocket.launch();
        panic!("Failed to launch rocket: {}", error);
    }
}

#[derive(Clone)]
struct GraphiqlHandler {
    graphql_path: &'static str,
}

impl GraphiqlHandler {
    fn new(graphql_path: &'static str) -> Self {
        Self { graphql_path }
    }
}

impl Handler for GraphiqlHandler {
    fn handle<'r>(&self, req: &'r Request, _: Data) -> handler::Outcome<'r> {
        let src = juniper_rocket::graphiql_source(self.graphql_path);
        Outcome::from(req, src)
    }
}

impl Into<Vec<Route>> for GraphiqlHandler {
    fn into(self) -> Vec<Route> {
        vec![Route::new(Method::Get, "/graphiql", self)]
    }
}

struct PostGraphqlHandler<Query, Mutation, Context> {
    query_type: PhantomData<Query>,
    mutation_type: PhantomData<Mutation>,
    context_type: PhantomData<Context>,
}

impl<Query, Mutation, Context> PostGraphqlHandler<Query, Mutation, Context> {
    fn new() -> Self {
        PostGraphqlHandler {
            query_type: PhantomData,
            mutation_type: PhantomData,
            context_type: PhantomData,
        }
    }
}

impl<Query, Mutation, Context> Clone for PostGraphqlHandler<Query, Mutation, Context> {
    fn clone(&self) -> Self {
        Self::new()
    }
}

unsafe impl<Query, Mutation, Context> Send for PostGraphqlHandler<Query, Mutation, Context> {}
unsafe impl<Query, Mutation, Context> Sync for PostGraphqlHandler<Query, Mutation, Context> {}

impl<Query, Mutation, Context> Handler for PostGraphqlHandler<Query, Mutation, Context>
where
    Query: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Mutation: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Context: 'static + juniper::Context + for<'ca, 'cr> FromRequest<'ca, 'cr>,
{
    fn handle<'r>(&self, req: &'r Request, data: Data) -> handler::Outcome<'r> {
        let context = match Context::from_request(req) {
            Outcome::Success(s) => s,
            Outcome::Forward(_) => return Outcome::Forward(data),
            Outcome::Failure((f, _)) => return Outcome::Failure(f),
        };

        let schema = match State::<RootNode<Query, Mutation>>::from_request(req) {
            Outcome::Success(s) => s,
            Outcome::Forward(_) => return Outcome::Forward(data),
            Outcome::Failure((f, _)) => return Outcome::Failure(f),
        };

        let transform = <GraphQLRequest as FromData>::transform(req, data);

        let outcome = match transform {
            Transform::Owned(Outcome::Success(v)) => Transform::Owned(Outcome::Success(v)),
            Transform::Borrowed(Outcome::Success(ref v)) => {
                Transform::Borrowed(Outcome::Success(Borrow::borrow(v)))
            }
            Transform::Borrowed(o) => Transform::Borrowed(o.map(|_| unreachable!())),
            Transform::Owned(o) => Transform::Owned(o),
        };

        let graphql_request = match <GraphQLRequest as FromData>::from_data(req, outcome) {
            Outcome::Success(s) => s,
            Outcome::Forward(f) => return Outcome::Forward(f),
            Outcome::Failure((f, _)) => return Outcome::Failure(f),
        };

        let responder = graphql_request.execute(&schema, &context);

        Outcome::from(req, responder)
    }
}

impl<Query, Mutation, Context> Into<Vec<Route>> for PostGraphqlHandler<Query, Mutation, Context>
where
    Query: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Mutation: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Context: 'static + juniper::Context + for<'ca, 'cr> FromRequest<'ca, 'cr>,
{
    fn into(self) -> Vec<Route> {
        vec![Route::new(Method::Post, "/graphql", self)]
    }
}

struct GetGraphqlHandler<Query, Mutation, Context> {
    query_type: PhantomData<Query>,
    mutation_type: PhantomData<Mutation>,
    context_type: PhantomData<Context>,
}

impl<Query, Mutation, Context> GetGraphqlHandler<Query, Mutation, Context> {
    fn new() -> Self {
        GetGraphqlHandler {
            query_type: PhantomData,
            mutation_type: PhantomData,
            context_type: PhantomData,
        }
    }
}

impl<Query, Mutation, Context> Clone for GetGraphqlHandler<Query, Mutation, Context> {
    fn clone(&self) -> Self {
        Self::new()
    }
}

unsafe impl<Query, Mutation, Context> Send for GetGraphqlHandler<Query, Mutation, Context> {}
unsafe impl<Query, Mutation, Context> Sync for GetGraphqlHandler<Query, Mutation, Context> {}

impl<Query, Mutation, Context> Handler for GetGraphqlHandler<Query, Mutation, Context>
where
    Query: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Mutation: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Context: 'static + juniper::Context + for<'ca, 'cr> FromRequest<'ca, 'cr>,
{
    fn handle<'r>(&self, req: &'r Request, data: Data) -> handler::Outcome<'r> {
        let context = match Context::from_request(req) {
            Outcome::Success(s) => s,
            Outcome::Forward(_) => return Outcome::Forward(data),
            Outcome::Failure((f, _)) => return Outcome::Failure(f),
        };

        let schema = match State::<RootNode<Query, Mutation>>::from_request(req) {
            Outcome::Success(s) => s,
            Outcome::Forward(_) => return Outcome::Forward(data),
            Outcome::Failure((f, _)) => return Outcome::Failure(f),
        };

        let mut graphql_request: Option<juniper_rocket::GraphQLRequest> = None;
        if let Some(items) = req.raw_query_items() {
            for item in items {
                match (item.key.as_str(), item.value) {
                    ("request", value) => {
                        let value = match <GraphQLRequest as FromFormValue>::from_form_value(value)
                        {
                            Ok(v) => v,
                            Err(e) => {
                                rocket::logger::warn(&format!(
                                    "Failed to parse 'request': {:?}",
                                    e
                                ));

                                return Outcome::Forward(data);
                            }
                        };
                        graphql_request = Some(value);
                    }
                    #[allow(unreachable_patterns, unreachable_code)]
                    _ => continue,
                }
            }
        }

        let graphql_request = match graphql_request
            .or_else(<juniper_rocket::GraphQLRequest as ::rocket::request::FromFormValue>::default)
        {
            Some(v) => v,
            None => {
                rocket::logger::warn_("Missing required query parameter 'request'.");
                return ::rocket::Outcome::Forward(data);
            }
        };

        let responder = graphql_request.execute(&schema, &context);

        Outcome::from(req, responder)
    }
}

impl<Query, Mutation, Context> Into<Vec<Route>> for GetGraphqlHandler<Query, Mutation, Context>
where
    Query: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Mutation: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Context: 'static + juniper::Context + for<'ca, 'cr> FromRequest<'ca, 'cr>,
{
    fn into(self) -> Vec<Route> {
        vec![Route::new(Method::Get, "/graphql", self)]
    }
}
