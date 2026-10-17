import asyncio
async def worker(worker_id):
    while True:
        task = await host.await_next_task(worker_id)
        if task is None:
            return
        token = task['task_token']
        host.enter_task(token)
        data = await host.read_file(token, task['user_input'])
        reference = await host.persist_result(token, data)
        await host.finish_task(token, {'status': 'internal_completed', 'receipt_ref': reference})
        task = None
        token = None
        data = None
        reference = None
async def coordinator():
    while True:
        update = await host.await_pool_update()
        if update is None:
            return
        active = set(update['active_ids'])
        for worker_id in range(update['workers']):
            if worker_id not in active:
                asyncio._replace_sibling(worker_id + 1, worker(worker_id))
                active.add(worker_id)
        for worker_id in update['borrowed_ids']:
            if worker_id not in active:
                asyncio._replace_sibling(worker_id + 1, worker(worker_id))
                active.add(worker_id)
async def main():
    await asyncio.gather(coordinator(), *[worker(i) for i in range(worker_count)])
asyncio.run(main())
