use crate::{DbConPool, WebFramework};
use juniper::{GraphQLType, RootNode};
use juniper_rocket::GraphQLRequest;
use rocket::{
    data::{FromData, Transform},
    handler::{self, Handler},
    http::Method,
    request::{FromRequest, Request},
    Data, Outcome, Route, State,
};
use std::borrow::Borrow;
use std::marker::PhantomData;

pub use rocket;

pub struct Rocket;

impl<Connection, Query, Mutation, Context> WebFramework<Connection, Query, Mutation, Context>
    for Rocket
where
    Connection: 'static + diesel::Connection,
    Query: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Mutation: 'static + Send + Sync + Default + GraphQLType<TypeInfo = (), Context = Context>,
    Context: 'static + juniper::Context + for<'ca, 'cr> FromRequest<'ca, 'cr>,
{
    fn new() -> Self {
        Rocket
    }

    fn run(&self, database_connection_pool: DbConPool<Connection>) {
        rocket::ignite()
            .manage(database_connection_pool)
            .manage(juniper::RootNode::new(
                Query::default(),
                Mutation::default(),
            ))
            .mount("/", GraphiqlHandler)
            .mount("/", PostGraphqlHandler::<Query, Mutation, Context>::new())
            .launch();
    }
}

#[derive(Clone)]
struct GraphiqlHandler;

impl Handler for GraphiqlHandler {
    fn handle<'r>(&self, req: &'r Request, _: Data) -> handler::Outcome<'r> {
        let src = juniper_rocket::graphiql_source("/graphql");
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

// TODO
// #[get("/graphql?<request>")]
// fn get_graphql_handler<'a, 'r, Ctx>(
//     context: Ctx,
//     request: juniper_rocket::GraphQLRequest,
//     schema: State<Schema>,
// ) -> juniper_rocket::GraphQLResponse
// where
//     Ctx: FromRequest<'a, 'r>,
// {
// }
