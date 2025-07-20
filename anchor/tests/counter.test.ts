import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey } from "@solana/web3.js";



describe('Sample', () => {
  const connection = new Connection("http://localhost:8899", "confirmed");

  let wallet = new PublicKey("86LBQoy8quHs94H6xZd8E17QbAmmtGY3ZnjzLoT4bHSV")
  let count = 0;
  
  
  const sign = async () => {
    const { blockhash , lastValidBlockHeight } = await connection.getLatestBlockhash()
    console.log("The latest blockhash is : ", blockhash)
    console.log("The last block height is : ", lastValidBlockHeight)
    return await connection.requestAirdrop(
      wallet,
      LAMPORTS_PER_SOL * 5
    )
  }
  
  it('Displaying', async () => {
    console.log("The signature of the Airdrop transaction is : ",await sign())
    console.log("The Airdrop was made to the public key : ",wallet)
  })

  expect(count).toEqual(0)
})


