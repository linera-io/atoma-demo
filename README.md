# Linera & Atoma demo

This is an example [Linera](https://linera.io) application that shows how to use the
[Atoma Network](https://atoma.network) to execute AI inference and verify it on-chain.

## Application Design

Any microchain can use the service to perform a query to the Atoma Network. The result is then
included in the operation that's added to the block, together with a certificate that the result
was produced by the Atoma Network. When the block is created, the contract is responsible for
checking the certificate.
