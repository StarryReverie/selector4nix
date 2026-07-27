# Testing

## Principles

Tests SHOULD strive to simultaneously satisfy all four of the following principles, as described in [Unit Testing: Principles, Practices, and Patterns](https://www.manning.com/books/unit-testing):

- Protection against regressions: The test catches bugs that would otherwise go unnoticed.
- Resistance to refactoring: The test does not produce false positives when the implementation changes without altering behavior.
- Fast feedback: The test executes quickly enough to provide timely feedback during development.
- Maintainability: The test is easy to understand and modify.

The value of a test is the product of these four factors. If any factor is zero, the test provides no value.

## Test Categories

The project has three categories of tests:

- Unit tests: In implementation modules via `#[cfg(test)]`. They verify a single unit of behavior in isolation.
- Integration tests: In the `tests/integration` crate inside each production sub-crate, with one module per test suite. They verify how units collaborate by assembling a vertical slice of the system into a virtual environment, mocking only the interactions at the system boundary.
- System tests: In the `tests/` directory, with one crate per test suite. They interact with the system entirely from the outside, exercising the full system end-to-end with real external dependencies managed by the test program.

The choice of test category is determined by the nature and responsibility of the code under test.

Unit tests SHOULD be used for core domain logic that is pure and free of side effects.

Integration tests SHOULD be used as soon as side effects or orchestration are involved. They verify a partial vertical slice of the system, assembling only the relevant components into a virtual environment with mocked system-boundary interactions. There is no need to do integration test for a feature's every layer, and the currently preference is only testing domain service or usecase.

System tests SHOULD be used to verify the full system end-to-end from an external perspective, with the test program managing real external dependencies and interacting through the system's external interfaces.

## Assertion Style

Three styles of assertion are used in this project:

- Output value verification: Checks the output of the code under test. This style is the most RECOMMENDED, as it couples the test only to the observable result and not to internal state or interactions. Do this type of verification as much as possible.
- State verification: Checks the state of the system after the code under test has executed. This is appropriate when the behavior under test produces side effects that manifest as observable state changes.
- Interaction verification: Checks how the code under test interacts with its collaborators, typically via mock expectations. This style SHOULD only be used at the system boundary, where interactions with uncontrolled external dependencies are being verified.

Assertions SHOULD NOT over-specify. Tests SHOULD verify only what is relevant to the behavior being tested. Beware of unintended coupling to details such as ordering when the order is not part of the observed behavior.

Assertions SHOULD NOT touch implementation details unless there is a compelling reason to perform white-box testing.

Tests MUST NOT duplicate the implementation logic. A test that reproduces the code under test provides no meaningful verification and is worse than having no test at all.

## Test Implementation

Fixtures and helper functions are RECOMMENDED to reduce duplication in the arrange phase and to protect tests from changes in constructor APIs.

For test suites that involve many data-driven cases, the relevant information of each case SHOULD be explicitly defined as dedicated structs, such as `TestCaseEnvironment`, `TestCaseInput`, and `TestCaseExpectation` in `tests/integration/nar_info_service_test.rs`. This unifies the arrange-act-assert flow into a single `run_test` function, so that individual test functions only need to declaratively fill in the case data.
